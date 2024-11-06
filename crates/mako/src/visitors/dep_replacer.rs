use std::collections::HashMap;
use std::sync::Arc;

use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    AssignOp, BlockStmt, Expr, ExprOrSpread, FnExpr, Function, Ident, ImportDecl, Lit, NamedExport,
    NewExpr, Stmt, Str, ThrowStmt, VarDeclKind,
};
use swc_core::ecma::utils::{member_expr, quote_ident, quote_str, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::file::parse_path;
use crate::ast::utils::{is_commonjs_require, is_dynamic_import, is_remote_or_data};
use crate::ast::DUMMY_CTXT;
use crate::compiler::Context;
use crate::module::{Dependency, ModuleId};
use crate::visitors::virtual_css_modules::is_css_path;

pub struct DepReplacer<'a> {
    pub module_id: &'a ModuleId,
    pub to_replace: &'a DependenciesToReplace,
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
}

#[derive(Debug, Clone)]
pub struct ResolvedReplaceInfo {
    pub chunk_id: Option<String>,
    pub to_replace_source: String,
    pub resolved_module_id: ModuleId,
}

#[derive(Debug, Clone)]
pub struct DependenciesToReplace {
    // resolved stores the "source" maps to (generate_id, raw_module_id)
    // e.g. "react" => ("hashed_id", "/abs/to/react/index.js")
    pub resolved: HashMap<String, ResolvedReplaceInfo>,
    pub missing: HashMap<String, Dependency>,
}

pub fn miss_throw_stmt<T: AsRef<str>>(source: T) -> Expr {
    // var e = new Error("Cannot find module '{source}'")
    let decl_error = quote_ident!("Error")
        .into_new_expr(
            DUMMY_SP,
            Some(vec![quote_str!(format!(
                "Cannot find module '{}'",
                source.as_ref()
            ))
            .as_arg()]),
        )
        .into_var_decl(VarDeclKind::Var, quote_ident!("e").into());

    // e.code = "MODULE_NOT_FOUND"
    let assign_error = quote_str!("MODULE_NOT_FOUND")
        .make_assign_to(
            AssignOp::Assign,
            member_expr!(DUMMY_CTXT, DUMMY_SP, e.code).into(),
        )
        .into_stmt();

    // function() { ...; throw e }
    let fn_expr = Expr::Fn(FnExpr {
        ident: Some(quote_ident!(DUMMY_CTXT, "makoMissingModule")),
        function: Box::new(Function {
            is_async: false,
            params: vec![],
            decorators: vec![],
            span: DUMMY_SP,
            ctxt: DUMMY_CTXT,
            body: Some(BlockStmt {
                ctxt: Default::default(),
                span: DUMMY_SP,
                stmts: vec![
                    decl_error.into(),
                    assign_error,
                    Stmt::Throw(ThrowStmt {
                        span: DUMMY_SP,
                        arg: quote_ident!("e").into(),
                    }),
                ],
            }),
            return_type: None,
            type_params: None,
            is_generator: false,
        }),
    });

    // (function() { ...; throw e;})()
    let iife = fn_expr.as_iife();

    // Object((function() { ...; throw e;})())
    quote_ident!("Object").as_call(DUMMY_SP, vec![iife.as_arg()])
}

impl VisitMut for DepReplacer<'_> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            let is_commonjs_require_flag = is_commonjs_require(call_expr, &self.unresolved_mark);
            let is_dynamic_import_flag = is_dynamic_import(call_expr);
            if is_commonjs_require_flag || is_dynamic_import_flag {
                if call_expr.args.is_empty() {
                    return;
                }
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    // commonjs require
                    let source_string = source.value.clone().to_string();

                    if !is_dynamic_import_flag {
                        match self.to_replace.missing.get(&source_string) {
                            Some(_) => {
                                call_expr.args[0] = ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(miss_throw_stmt(&source_string)),
                                };
                                return;
                            }
                            None => {
                                self.replace_source(source);
                            }
                        }
                    }

                    // css
                    // TODO: add testcases for this
                    let is_replaceable_css =
                        if let Some(replace_info) = self.to_replace.resolved.get(&source_string) {
                            let (path, _search, query, _) =
                                parse_path(&replace_info.resolved_module_id.id).unwrap();
                            // when inline_css is enabled
                            // css is parsed as js modules
                            self.context.config.inline_css.is_none()
                                && is_css_path(&path)
                                && (query.is_empty() || query.iter().any(|(k, _)| *k == "modules"))
                        } else {
                            false
                        };
                    if is_replaceable_css {
                        // remove `require('./xxx.css');`
                        if is_commonjs_require_flag {
                            *expr = Expr::Lit(quote_str!("").into());
                            return;
                        } else {
                            // `import('./xxx.css')` 中的 css 模块会被拆分到单独的 chunk, 这里需要改为加载 css chunk
                            let module_graph = self.context.module_graph.read().unwrap();
                            let dep_module_id = module_graph
                                .get_dependency_module_by_source(self.module_id, &source_string);

                            if let Some(dep_module_id) = dep_module_id {
                                let chunk_graph = self.context.chunk_graph.read().unwrap();
                                let chunk =
                                    chunk_graph.get_chunk_for_module(&dep_module_id.clone());

                                if let Some(chunk) = chunk {
                                    let chunk_id = chunk.id.id.clone();
                                    // `import('./xxx.css')` => `__mako_require__.ensure('./xxx.css')`
                                    *expr =
                                        member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_require__.ensure)
                                            .as_call(DUMMY_SP, vec![quote_str!(chunk_id).as_arg()]);
                                    return;
                                } else {
                                    *expr = Expr::Lit(quote_str!("").into());
                                    return;
                                }
                            } else {
                                *expr = Expr::Lit(quote_str!("").into());
                                return;
                            }
                        }
                    }
                }
            }
        }
        expr.visit_mut_children_with(self);
    }

    fn visit_mut_new_expr(&mut self, new_expr: &mut NewExpr) {
        if let Some(str) = resolve_web_worker_mut(new_expr, self.unresolved_mark) {
            self.replace_source(str);
        }
        new_expr.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        self.replace_source(&mut import_decl.src);
    }

    fn visit_mut_named_export(&mut self, n: &mut NamedExport) {
        if let Some(ref mut src) = n.src {
            self.replace_source(src.as_mut());
        }
    }
}

impl DepReplacer<'_> {
    fn replace_source(&mut self, source: &mut Str) {
        if let Some(replacement) = self.to_replace.resolved.get(&source.value.to_string()) {
            let module_id = replacement.to_replace_source.clone();
            let span = source.span;
            *source = Str::from(module_id);
            source.span = span;
        }
    }
}

// TODO: duplicated code with dep_analyzer.rs
pub fn resolve_web_worker_mut(new_expr: &mut NewExpr, unresolved_mark: Mark) -> Option<&mut Str> {
    if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty()) || !new_expr.callee.is_ident() {
        return None;
    }

    if let box Expr::Ident(Ident { sym, ctxt, .. }) = &mut new_expr.callee {
        // `Worker` must be unresolved
        if sym == "Worker" && (ctxt.outer() == unresolved_mark) {
            let args = new_expr.args.as_mut().unwrap();

            // new Worker(new URL(''), base);
            if let Expr::New(new_expr) = &mut *args[0].expr {
                if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty())
                    || !new_expr.callee.is_ident()
                {
                    return None;
                }

                if let box Expr::Ident(Ident { sym, ctxt, .. }) = &new_expr.callee {
                    if sym == "URL" && (ctxt.outer() == unresolved_mark) {
                        // new URL('');
                        let args = new_expr.args.as_mut().unwrap();
                        if let box Expr::Lit(Lit::Str(ref mut str)) = &mut args[0].expr {
                            if !is_remote_or_data(&str.value) {
                                return Some(str);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use maplit::hashmap;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::{DepReplacer, DependenciesToReplace, ResolvedReplaceInfo};
    use crate::ast::tests::TestUtils;
    use crate::module::{Dependency, ImportType, ModuleId, ResolveType};

    #[test]
    fn test_require() {
        assert_eq!(
            run(
                r#"require("react");"#,
                build_resolved("react", "/root/node_modules/react/index.js"),
                Default::default()
            ),
            r#"require("/root/node_modules/react/index.js");"#,
        );
    }

    #[test]
    fn test_import() {
        assert_eq!(
            run(
                r#"import x from "x";"#,
                build_resolved("x", "/x/index.js"),
                Default::default()
            ),
            r#"import x from "/x/index.js";"#,
        );
    }

    #[test]
    fn test_export() {
        assert_eq!(
            run(
                r#"export {x} from "x";"#,
                build_resolved("x", "/x/index.js"),
                Default::default()
            ),
            r#"export { x } from "/x/index.js";"#,
        );
    }

    #[test]
    fn test_worker() {
        assert_eq!(
            run(
                r#"new Worker(new URL('x'), base)"#,
                build_resolved("x", "/x/index.js"),
                Default::default()
            ),
            r#"new Worker(new URL("/x/index.js"), base);"#,
        );
    }

    #[test]
    fn test_missing_dep() {
        assert_eq!(
            run(
                r#"require("react");"#,
                Default::default(),
                build_missing("react"),
            ),
            r#"
require(Object(function makoMissingModule() {
    var e = new Error("Cannot find module 'react'");
    e.code = "MODULE_NOT_FOUND";
    throw e;
}()));
            "#
            .trim(),
        );
    }

    #[test]
    fn test_missing_dep_in_try_top_level() {
        assert_eq!(
            run(
                r#"try{require("react")}catch(e){}"#,
                Default::default(),
                build_missing("react"),
            ),
            r#"
try {
    require(Object(function makoMissingModule() {
        var e = new Error("Cannot find module 'react'");
        e.code = "MODULE_NOT_FOUND";
        throw e;
    }()));
} catch (e) {}
            "#
            .trim(),
        );
    }

    fn build_resolved(key: &str, module_id: &str) -> HashMap<String, ResolvedReplaceInfo> {
        hashmap! {
            key.to_string() =>
            ResolvedReplaceInfo {
                chunk_id: None,
                to_replace_source: module_id.into(),
                resolved_module_id: "".into(),
            }
        }
    }

    fn build_missing(key: &str) -> HashMap<String, Dependency> {
        hashmap! {
            key.to_string() => Dependency {
                resolve_type: ResolveType::Import(ImportType::Default),
                source: key.to_string(),
                resolve_as: None,
                span: None,
                order: 0,
            }
        }
    }

    fn run(
        js_code: &str,
        resolved: HashMap<String, ResolvedReplaceInfo>,
        missing: HashMap<String, Dependency>,
    ) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DepReplacer {
                module_id: &ModuleId::new("index.jsx".into()),
                to_replace: &DependenciesToReplace { resolved, missing },
                context: &test_utils.context,
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
