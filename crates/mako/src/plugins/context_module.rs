use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use glob::glob;
use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    BinExpr, BinaryOp, CallExpr, Expr, ExprOrSpread, Lit, ParenExpr, TplElement,
};
use swc_core::ecma::utils::{member_expr, quote_ident, quote_str, ExprExt, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::file::{Content, JsContent};
use crate::ast::utils::{is_commonjs_require, is_dynamic_import};
use crate::ast::DUMMY_CTXT;
use crate::build::load::JS_EXTENSIONS;
use crate::compiler::Context;
use crate::plugin::{Plugin, PluginLoadParam};
use crate::resolve::get_module_extensions;

pub struct ContextModulePlugin {}

impl Plugin for ContextModulePlugin {
    fn name(&self) -> &str {
        "context_module"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if let (Some(glob_pattern), true) = (
            param
                .file
                .params
                .iter()
                .find_map(|(k, v)| k.eq("glob").then_some(v)),
            param.file.pathname.is_dir(),
        ) {
            let glob_pattern = param.file.pathname.clone().join(glob_pattern);
            let paths = glob(glob_pattern.to_str().unwrap())?;

            let mut key_values = BTreeMap::new();
            let load_by = if param.file.has_param("async") {
                "import"
            } else {
                "require"
            };

            for path in paths {
                let path = path?;
                let rlt_path = path.strip_prefix(&param.file.pathname)?;
                let is_file = path.is_file();

                // full path `./i18n/jzh_CN.json`
                let mut keys = HashSet::new();

                let metadata = fs::metadata(&path);
                if let Ok(md) = metadata {
                    if md.is_dir() && !has_index_file_in_directory(&path) {
                        continue;
                    }
                }
                keys.insert(format!("./{}", rlt_path.to_string_lossy()));
                // omit ext `./i18n/zh_CN`
                if let Some(ext) = rlt_path.extension() {
                    if is_file
                        && get_module_extensions().contains(&format!(".{}", ext.to_string_lossy()))
                    {
                        keys.insert(format!(
                            "./{}",
                            rlt_path.with_extension("").to_string_lossy()
                        ));

                        // entry file `./i18n/`, `./i18n`, `.`, `./`
                        if rlt_path.file_stem().unwrap() == "index" {
                            let entry_paths = rlt_path
                                .parent()
                                .map(|p| {
                                    let parent = p.to_string_lossy().to_string();

                                    parent
                                        .is_empty()
                                        // root entry
                                        .then(|| vec![".".to_string(), "./".to_string()])
                                        // non-root entry
                                        .unwrap_or(vec![
                                            format!("./{}", parent),
                                            format!("./{}/", parent),
                                        ])
                                })
                                .unwrap();

                            keys.extend(entry_paths);
                        }
                    }
                }

                for key in keys {
                    let map_entry =
                        format!("'{}': () => {}('{}')", key, load_by, path.to_string_lossy());

                    key_values.insert(key, map_entry);
                }
            }

            let content = format!(
                r#"
const map = {{
    {}
}};

module.exports = (id) => {{
    if (map[id]) return map[id]();
    else {{
        const e = new Error("Cannot find module '" + id + "'");
        e.code = 'MODULE_NOT_FOUND';
        throw e;
    }}
}};
"#,
                key_values
                    .into_values()
                    .collect::<Vec<String>>()
                    .join(",\n")
            );
            Ok(Some(Content::Js(JsContent {
                content,
                ..Default::default()
            })))
        } else {
            Ok(None)
        }
    }
}

pub struct ContextModuleVisitor {
    pub unresolved_mark: Mark,
}

impl VisitMut for ContextModuleVisitor {
    fn visit_mut_call_expr(&mut self, expr: &mut CallExpr) {
        let commonjs_require = is_commonjs_require(expr, &self.unresolved_mark);
        let dynamic_import = is_dynamic_import(expr);
        let first_non_str_arg = match expr.args.first_mut() {
            Some(ExprOrSpread {
                expr: box Expr::Lit(Lit::Str(_)),
                ..
            }) => None,
            Some(ExprOrSpread { expr, .. }) => Some(expr),
            _ => None,
        };

        if (commonjs_require || dynamic_import) && first_non_str_arg.is_some() {
            if let Some((from, glob)) = try_replace_context_arg(
                &mut *first_non_str_arg.unwrap(),
                false,
            )
            .map(|(prefix, suffix)| (prefix, format!("**/*{}", suffix.unwrap_or("".to_string()),)))
            {
                let args_literals = format!("{}?context&glob={}", from, glob);

                let mut ctxt_call_expr = CallExpr {
                    ctxt: Default::default(),
                    callee: expr.callee.clone(),
                    args: vec![quote_str!(args_literals.clone()).as_arg()],
                    span: DUMMY_SP,
                    type_args: None,
                };

                if commonjs_require {
                    // require('./i18n' + n) -> require('./i18n?context&glob=**/*')('.' + n)
                    expr.callee = ctxt_call_expr.as_callee();
                } else {
                    // mark async import in params
                    ctxt_call_expr.args =
                        vec![quote_str!(format!("{}&{}", args_literals, "async")).as_arg()];

                    // import('./i18n' + n) -> import('./i18n?context&glob=**/*').then(m => m('.' + n))
                    expr.callee = member_expr!(
                        @EXT,
                        DUMMY_SP,
                        ctxt_call_expr.into(),
                        then
                    )
                    .as_callee();
                    // TODO: allow use await in args
                    // eg: import(`./i18n${await xxx()}`)
                    expr.args = vec![member_expr!(DUMMY_CTXT, DUMMY_SP, m.default)
                        .as_call(DUMMY_SP, expr.args.clone())
                        .as_expr()
                        .to_owned()
                        .into_lazy_arrow(vec![quote_ident!("m").into()])
                        .as_arg()]
                }
            }
        }

        expr.visit_mut_children_with(self);
    }
}

/**
 * try to find valid context arg
 * and return prefix, suffix and replace first string literal with `./`
 * why we need to replace with `./` prefix?
 * because the context module map is a relative path map, to reduce bundle size
 */
fn try_replace_context_arg(
    mut o_expr: &mut Expr,
    has_visit_top_bin: bool,
) -> Option<(String, Option<String>)> {
    match &mut o_expr {
        // handle `(...)`
        Expr::Paren(ParenExpr {
            expr: paren_expr, ..
        }) => try_replace_context_arg(paren_expr, has_visit_top_bin),

        // handle `'./foo/' + bar`
        Expr::Bin(BinExpr {
            op: BinaryOp::Add,
            right: right_expr,
            left: left_expr,
            ..
        }) => {
            // handle suffix of `'./foo/' + bar + '.ext'`
            try_replace_context_arg(left_expr, true).map(|(prefix, _)| {
                let suffix =
                    if let (Expr::Lit(Lit::Str(str)), false) = (&**right_expr, has_visit_top_bin) {
                        Some(str.value.to_string())
                    } else {
                        None
                    };

                (prefix, suffix)
            })
        }

        // handle prefix of `'./foo/' + bar + '.ext'`
        Expr::Lit(Lit::Str(str)) => {
            let mut prefix = str.value.to_string();
            // replace first str with relative prefix
            let (pre_quasis, remainder) = if let Some(pos) = prefix.rfind('/') {
                (prefix[..=pos].to_string(), prefix[pos + 1..].to_string())
            } else {
                (prefix.clone(), String::new())
            };
            str.value = format!("./{}", remainder).into();
            str.raw = None;
            if !prefix.ends_with('/') {
                prefix = pre_quasis;
            }
            Some((prefix, None))
        }

        // handle `./foo/${bar}.ext`
        //        `${bar}` will be handle as `./${bar}`
        Expr::Tpl(tpl) => {
            if !tpl.exprs.is_empty() {
                let first_quasis_str = tpl.quasis.first().unwrap().raw.to_string();
                let pre_quasis = if first_quasis_str.is_empty() {
                    "./".to_string()
                } else {
                    first_quasis_str
                };

                let (prefix, remainder) = if let Some(pos) = pre_quasis.rfind('/') {
                    (
                        pre_quasis[..=pos].to_string(),
                        pre_quasis[pos + 1..].to_string(),
                    )
                } else {
                    (pre_quasis.clone(), String::new())
                };
                let mut suffix = None;

                // replace first quasi with relative prefix
                tpl.quasis[0].raw = format!("./{}", remainder).into();
                tpl.quasis[0].cooked = None;

                // extract suffix
                if tpl.quasis.len() > 1 {
                    if let Some(TplElement { raw, .. }) = tpl.quasis.last() {
                        suffix = Some(raw.to_string());
                    }
                }
                Some((prefix, suffix))
            } else {
                None
            }
        }

        _ => None,
    }
}

fn has_index_file_in_directory(dir_path: &Path) -> bool {
    fs::read_dir(dir_path)
        .map(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                let path = entry.path();
                path.is_file()
                    && path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .map_or(false, |fname| fname == "index")
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |extension: &str| JS_EXTENSIONS.contains(&extension))
            })
        })
        .unwrap_or(false)
}
