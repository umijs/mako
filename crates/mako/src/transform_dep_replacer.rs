use std::collections::HashMap;
use std::sync::Arc;

use swc_atoms::js_word;
use swc_common::DUMMY_SP;
use swc_ecma_ast::Expr::Call;
use swc_ecma_ast::{
    AssignExpr, AssignOp, BindingIdent, BlockStmt, CallExpr, Callee, Decl, Expr, ExprOrSpread,
    ExprStmt, FnExpr, Function, Ident, Lit, MemberExpr, MemberProp, NamedExport, NewExpr, Pat,
    PatOrExpr, Stmt, Str, ThrowStmt, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_ecma_utils::quote_ident;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::{is_commonjs_require, is_dynamic_import};
use crate::compiler::Context;
use crate::module::Dependency;

pub struct DepReplacer<'a> {
    pub to_replace: &'a DependenciesToReplace,
    pub context: &'a Arc<Context>,
}

#[derive(Debug, Clone)]
pub struct DependenciesToReplace {
    pub resolved: HashMap<String, String>,
    pub missing: HashMap<String, Dependency>,
}

fn miss_throw_stmt<T: AsRef<str>>(source: T) -> Expr {
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
                    Stmt::Decl(Decl::Var(Box::new(VarDecl {
                        span: DUMMY_SP,
                        kind: VarDeclKind::Var,
                        declare: false,
                        decls: vec![VarDeclarator {
                            span: DUMMY_SP,
                            name: Pat::Ident(BindingIdent {
                                id: quote_ident!("e"),
                                type_ann: None,
                            }),
                            init: Some(Box::new(Expr::New(NewExpr {
                                span: DUMMY_SP,
                                callee: Box::new(Expr::Ident(Ident::new(
                                    js_word!("Error"),
                                    DUMMY_SP,
                                ))),
                                args: Some(vec![ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                                        span: DUMMY_SP,
                                        value: format!("Cannot find module '{}'", source.as_ref())
                                            .into(),
                                        raw: None,
                                    }))),
                                }]),
                                type_args: None,
                            }))),
                            definite: false,
                        }],
                    }))),
                    Stmt::Expr(ExprStmt {
                        span: DUMMY_SP,
                        expr: Box::new(Expr::Assign(AssignExpr {
                            span: DUMMY_SP,
                            left: PatOrExpr::Expr(Box::new(Expr::Member(MemberExpr {
                                span: DUMMY_SP,
                                obj: Box::new(Expr::Ident(quote_ident!("e"))),
                                prop: MemberProp::Ident(quote_ident!("code")),
                            }))),
                            op: AssignOp::Assign,
                            right: Box::new(Expr::Lit(Lit::Str(Str {
                                span: DUMMY_SP,
                                value: "MODULE_NOT_FOUND".into(),
                                raw: None,
                            }))),
                        })),
                    }),
                    Stmt::Throw(ThrowStmt {
                        span: DUMMY_SP,
                        arg: Box::new(Expr::Ident(quote_ident!("e"))),
                    }),
                ],
            }),
            return_type: None,
            type_params: None,
            is_generator: false,
        }),
    });

    Call(CallExpr {
        span: DUMMY_SP,
        callee: Callee::Expr(Box::new(Expr::Ident(quote_ident!("Object")))),
        args: vec![ExprOrSpread {
            spread: None,
            expr: Box::new(Call(CallExpr {
                span: DUMMY_SP,
                args: vec![],
                callee: Callee::Expr(Box::new(fn_expr)),
                type_args: None,
            })),
        }],
        type_args: None,
    })
}

impl VisitMut for DepReplacer<'_> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_commonjs_require(call_expr) || is_dynamic_import(call_expr) {
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
                }
            }
        }
        expr.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut swc_ecma_ast::ImportDecl) {
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
            let span = source.span;

            let module_id_string = replacement.clone();
            //generate_module_id(replacement.clone(), self.context);

            // NOTE: JsWord 有缓存，直接设置 value 的方式在这种情况下不会生效
            // if (process.env.NODE_ENV === 'development') { require("./foo") }
            *source = Str::from(module_id_string);
            // 保持原来的 span，不确定不加的话会不会导致 sourcemap 错误

            source.span = span;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use maplit::hashmap;
    use swc_common::GLOBALS;
    use swc_ecma_visit::VisitMut;

    use crate::assert_display_snapshot;
    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::module::{Dependency, ResolveType};
    use crate::test_helper::transform_ast_with;
    use crate::transform_dep_replacer::{DepReplacer, DependenciesToReplace};

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
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
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
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
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
            };

            let cloned = context.clone();
            let mut visitor: Box<dyn VisitMut> = Box::new(DepReplacer {
                to_replace: &to_replace,
                context: &cloned,
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
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("test.js", code, &context).unwrap();

            let mut visitor = DepReplacer {
                to_replace: &DependenciesToReplace {
                    resolved: hashmap! {
                        "x".to_string() => "/x/index.js".to_string()
                    },
                    missing: hashmap! {},
                },
                context: &context,
            };

            transform_ast_with(&mut ast.ast, &mut visitor, &context.meta.script.cm)
        })
    }
}
