use std::collections::HashMap;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{
    AssignOp, BlockStmt, Expr, ExprOrSpread, FnExpr, Function, Ident, ImportDecl, Lit, NamedExport,
    NewExpr, Stmt, Str, ThrowStmt, VarDeclKind,
};
use mako_core::swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast_2::utils::{is_commonjs_require, is_dynamic_import, is_remote_or_data};
use crate::compiler::Context;
use crate::module::{Dependency, ModuleId};
use crate::transformers::transform_virtual_css_modules::is_css_path;

pub struct DepReplacer<'a> {
    pub module_id: &'a ModuleId,
    pub to_replace: &'a DependenciesToReplace,
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
    pub top_level_mark: Mark,
}

#[derive(Debug, Clone)]
pub struct DependenciesToReplace {
    // resolved stores the "source" maps to (generate_id, raw_module_id)
    // e.g. "react" => ("hashed_id", "/abs/to/react/index.js")
    pub resolved: HashMap<String, (String, String)>,
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
        .make_assign_to(AssignOp::Assign, member_expr!(DUMMY_SP, e.code).into())
        .into_stmt();

    // function() { ...; throw e }
    let fn_expr = Expr::Fn(FnExpr {
        ident: Some(quote_ident!("makoMissingModule")),
        function: Box::new(Function {
            is_async: false,
            params: vec![],
            decorators: vec![],
            span: DUMMY_SP,
            body: Some(BlockStmt {
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
            if is_commonjs_require_flag || is_dynamic_import(call_expr) {
                if call_expr.args.is_empty() {
                    return;
                }
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    let source_string = source.value.clone().to_string();

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

                    let is_dep_replaceable = if let Some((_, raw_id)) =
                        self.to_replace.resolved.get(&source_string)
                    {
                        let file_request = parse_path(raw_id).unwrap();
                        // when inline_css is enabled
                        // css is parsed as js modules
                        self.context.config.inline_css.is_none()
                            && is_css_path(&file_request.path)
                            && (file_request.query.is_empty() || file_request.has_query("modules"))
                    } else {
                        false
                    };

                    if is_dep_replaceable {
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
                                    *expr = member_expr!(DUMMY_SP, __mako_require__.ensure)
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
            let module_id = replacement.0.clone();
            let span = source.span;
            *source = Str::from(module_id);
            source.span = span;
        }
    }
}

pub fn resolve_web_worker_mut(new_expr: &mut NewExpr, unresolved_mark: Mark) -> Option<&mut Str> {
    if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty()) || !new_expr.callee.is_ident() {
        return None;
    }

    if let box Expr::Ident(Ident { span, sym, .. }) = &mut new_expr.callee {
        // `Worker` must be unresolved
        if sym == "Worker" && (span.ctxt.outer() == unresolved_mark) {
            let args = new_expr.args.as_mut().unwrap();

            // new Worker(new URL(''), base);
            if let Expr::New(new_expr) = &mut *args[0].expr {
                if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty())
                    || !new_expr.callee.is_ident()
                {
                    return None;
                }

                if let box Expr::Ident(Ident { span, sym, .. }) = &new_expr.callee {
                    if sym == "URL" && (span.ctxt.outer() == unresolved_mark) {
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

// TODO: REMOVE THIS, pass file to visitor instead
fn parse_path(path: &str) -> Result<FileRequest> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    Ok(FileRequest {
        path: path.to_string(),
        query: query_vec,
    })
}

#[derive(Debug, Clone)]
pub struct FileRequest {
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl FileRequest {
    pub fn has_query(&self, key: &str) -> bool {
        self.query.iter().any(|(k, _)| *k == key)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use mako_core::swc_common::{chain, GLOBALS};
    use mako_core::swc_ecma_transforms::resolver;
    use mako_core::swc_ecma_visit::VisitMut;
    use maplit::hashmap;

    use crate::assert_display_snapshot;
    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::module::{Dependency, Module, ModuleId, ResolveType};
    use crate::test_helper::transform_ast_with;
    use crate::transformers::test_helper::transform_js_code;
    use crate::transformers::transform_dep_replacer::{DepReplacer, DependenciesToReplace};

    #[test]
    fn test_simple_replace() {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast =
                build_js_ast("index.jsx", r#"require("react")"#, &context.clone()).unwrap();

            let to_replace = DependenciesToReplace {
                resolved: hashmap! {"react".to_string()=>
                    (
                        "/root/node_modules/react/index.js".to_string(),
                        "/root/node_modules/react/index.js".to_string()
                    )
                },
                missing: HashMap::new(),
            };

            let cloned = context.clone();

            cloned.module_graph.write().unwrap().add_module(Module {
                id: "index.jsx".to_string().into(),
                is_entry: false,
                info: None,
                side_effects: false,
            });

            let module_id = ModuleId::new("index.jsx".to_string());
            let mut visitor: Box<dyn VisitMut> = Box::new(chain!(
                resolver(ast.unresolved_mark, ast.top_level_mark, false),
                DepReplacer {
                    module_id: &module_id,
                    to_replace: &to_replace,
                    context: &cloned,
                    unresolved_mark: ast.unresolved_mark,
                    top_level_mark: ast.top_level_mark,
                }
            ));

            assert_display_snapshot!(transform_ast_with(
                &mut ast.ast,
                &mut visitor,
                &context.meta.script.cm
            ));
        });
    }

    #[test]
    fn test_replace_missing_dep() {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast =
                build_js_ast("index.jsx", r#"require("react")"#, &context.clone()).unwrap();

            let to_replace = DependenciesToReplace {
                resolved: HashMap::new(),
                missing: hashmap! {"react".to_string() => Dependency {
                    resolve_type: ResolveType::Import,
                    source: "react".to_string(),
                    resolve_as: None,
                    span: None,
                    order: 0,
                }},
            };

            let cloned = context.clone();
            let module_id = ModuleId::new("index.jsx".to_string());
            let mut visitor: Box<dyn VisitMut> = Box::new(chain!(
                resolver(ast.unresolved_mark, ast.top_level_mark, false),
                DepReplacer {
                    module_id: &module_id,
                    to_replace: &to_replace,
                    context: &cloned,
                    unresolved_mark: ast.unresolved_mark,
                    top_level_mark: ast.top_level_mark,
                }
            ));

            assert_display_snapshot!(transform_ast_with(
                &mut ast.ast,
                &mut visitor,
                &context.meta.script.cm
            ));
        });
    }

    #[test]
    fn test_replace_top_level_missing_dep_in_try() {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast(
                "index.jsx",
                r#"
                                       try {require("react")}catch(e){}"#,
                &context.clone(),
            )
            .unwrap();

            let to_replace = DependenciesToReplace {
                resolved: HashMap::new(),
                missing: hashmap! {"react".to_string() => Dependency {
                    resolve_type: ResolveType::Import,
                    source: "react".to_string(),
                    resolve_as: None,
                    span: None,
                    order: 0,
                }},
            };

            let cloned = context.clone();
            let module_id = ModuleId::new("index.jsx".to_string());
            let mut visitor: Box<dyn VisitMut> = Box::new(chain!(
                resolver(ast.unresolved_mark, ast.top_level_mark, false),
                DepReplacer {
                    module_id: &module_id,
                    to_replace: &to_replace,
                    context: &cloned,
                    unresolved_mark: ast.unresolved_mark,
                    top_level_mark: ast.top_level_mark,
                }
            ));

            assert_display_snapshot!(transform_ast_with(
                &mut ast.ast,
                &mut visitor,
                &context.meta.script.cm
            ));
        });
    }

    #[test]
    fn test_import_replace() {
        assert_display_snapshot!(transform_code("import x from 'x'"));
    }

    #[test]
    fn test_export_from_replace() {
        assert_display_snapshot!(transform_code("export {x} from 'x'"));
    }

    #[test]
    fn test_dynamic_import_from_replace() {
        assert_display_snapshot!(transform_code("const x = import('x')"));
    }

    fn transform_code(code: &str) -> String {
        let context: Arc<Context> = Arc::new(Default::default());
        let unresolved_mark = Default::default();
        let top_level_mark = Default::default();

        let mut visitor = DepReplacer {
            module_id: &ModuleId::new("index.jsx".into()),
            to_replace: &DependenciesToReplace {
                resolved: hashmap! {
                    "x".to_string() =>
                    (

                     "/x/index.js".to_string(),
                     "/x/index.js".to_string()
                    )
                },
                missing: hashmap! {},
            },
            context: &context,
            unresolved_mark,
            top_level_mark,
        };
        transform_js_code(code, &mut visitor, &context)
    }
}
