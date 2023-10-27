use std::collections::HashMap;
use std::sync::Arc;

use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{
    AssignOp, BlockStmt, Expr, ExprOrSpread, FnExpr, Function, ImportDecl, Lit, NamedExport,
    NewExpr, Stmt, Str, ThrowStmt, VarDeclKind,
};
use mako_core::swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::build::parse_path;
use crate::compiler::Context;
use crate::module::Dependency;
use crate::plugins::javascript::{is_commonjs_require, is_dynamic_import, is_web_worker};
use crate::transformers::transform_virtual_css_modules::is_css_path;

pub struct DepReplacer<'a> {
    pub to_replace: &'a DependenciesToReplace,
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
    pub top_level_mark: Mark,
}

#[derive(Debug, Clone)]
pub struct DependenciesToReplace {
    pub resolved: HashMap<String, String>,
    pub missing: HashMap<String, Dependency>,
    pub ignored: Vec<String>,
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
            if is_commonjs_require(call_expr, None) || is_dynamic_import(call_expr) {
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
                        }
                        None => {
                            self.replace_source(source);
                        }
                    }

                    // remove `require('./xxx.css');`
                    let file_request = parse_path(&source_string).unwrap();
                    if is_css_path(&file_request.path)
                        && (file_request.query.is_empty() || file_request.has_query("modules"))
                    {
                        *expr = Expr::Lit(quote_str!("").into())
                    }
                }
            }
        }
        expr.visit_mut_children_with(self);
    }

    fn visit_mut_new_expr(&mut self, new_expr: &mut NewExpr) {
        if is_web_worker(new_expr, self.unresolved_mark) {
            let args = new_expr.args.as_mut().unwrap();
            if let box Expr::Lit(Lit::Str(ref mut str)) = &mut args[0].expr {
                self.replace_source(str);
            }
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
        let to_replace =
            if let Some(replacement) = self.to_replace.resolved.get(&source.value.to_string()) {
                replacement.clone()
            } else if self.to_replace.ignored.contains(&source.value.to_string()) {
                "$$IGNORED$$".to_string()
            } else {
                return;
            };

        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_visit::VisitMut;
    use maplit::hashmap;

    use crate::assert_display_snapshot;
    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::module::{Dependency, ResolveType};
    use crate::test_helper::transform_ast_with;
    use crate::transformers::test_helper::transform_js_code;
    use crate::transformers::transform_dep_replacer::{DepReplacer, DependenciesToReplace};

    #[test]
    fn test_simple_replace() {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("index.jsx",
                                       r#"require("react")"#
                                       , &context.clone()).unwrap();

            let to_replace  = DependenciesToReplace {
                resolved: hashmap! {"react".to_string()=> "/root/node_modules/react/index.js".to_string()},
                missing: HashMap::new(),
                ignored: vec![],
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
                unresolved_mark: ast.unresolved_mark,
                top_level_mark: ast.top_level_mark,
            });

            assert_display_snapshot!(transform_ast_with(&mut ast.ast, &mut visitor, &context.meta.script.cm));
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
                    span: None,
                    order: 0,
                }},
                ignored: vec![],
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
                unresolved_mark: ast.unresolved_mark,
                top_level_mark: ast.top_level_mark,
            });

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
                    span: None,
                    order: 0,
                }},
                ignored: vec![],
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
                unresolved_mark: ast.unresolved_mark,
                top_level_mark: ast.top_level_mark,
            });

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
        let context = Arc::new(Default::default());
        let mut visitor = DepReplacer {
            to_replace: &DependenciesToReplace {
                resolved: hashmap! {
                    "x".to_string() => "/x/index.js".to_string()
                },
                missing: hashmap! {},
                ignored: vec![],
            },
            context: &context,
            unresolved_mark: Default::default(),
            top_level_mark: Default::default(),
        };
        transform_js_code(code, &mut visitor, &context)
    }
}
