use std::sync::Arc;

use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, MemberExpr, MemberProp,
};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::is_dynamic_import;
use crate::compiler::Context;
use crate::module::generate_module_id;

pub struct DynamicImport<'a> {
    pub context: &'a Arc<Context>,
}

impl VisitMut for DynamicImport<'_> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_dynamic_import(call_expr) {
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    let module_id =
                        generate_module_id(source.value.clone().to_string(), self.context);
                    // require.ensure(["id"]).then(require.bind(require, "id"))
                    *expr = Expr::Call(CallExpr {
                        span: DUMMY_SP,
                        callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
                            span: DUMMY_SP,
                            obj: Box::new(Expr::Call(CallExpr {
                                span: DUMMY_SP,
                                callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
                                    span: DUMMY_SP,
                                    obj: Box::new(Expr::Ident(Ident {
                                        span: DUMMY_SP,
                                        sym: "require".into(),
                                        optional: false,
                                    })),
                                    prop: MemberProp::Ident(Ident {
                                        span: DUMMY_SP,
                                        sym: "ensure".into(),
                                        optional: false,
                                    }),
                                }))),
                                args: vec![ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(Expr::Array(ArrayLit {
                                        span: DUMMY_SP,
                                        elems: vec![Some(ExprOrSpread {
                                            spread: None,
                                            expr: Box::new(Expr::Lit(Lit::Str(
                                                module_id.clone().into(),
                                            ))),
                                        })],
                                    })),
                                }],
                                type_args: None,
                            })),
                            prop: MemberProp::Ident(Ident {
                                span: DUMMY_SP,
                                sym: "then".into(),
                                optional: false,
                            }),
                        }))),
                        args: vec![ExprOrSpread {
                            spread: None,
                            expr: Box::new(Expr::Call(CallExpr {
                                span: DUMMY_SP,
                                callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
                                    span: DUMMY_SP,
                                    obj: Box::new(Expr::Ident(Ident {
                                        span: DUMMY_SP,
                                        sym: "require".into(),
                                        optional: false,
                                    })),
                                    prop: MemberProp::Ident(Ident {
                                        span: DUMMY_SP,
                                        sym: "bind".into(),
                                        optional: false,
                                    }),
                                }))),
                                args: vec![
                                    ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Ident(Ident {
                                            span: DUMMY_SP,
                                            sym: "require".into(),
                                            optional: false,
                                        })),
                                    },
                                    ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Lit(Lit::Str(module_id.into()))),
                                    },
                                ],
                                type_args: None,
                            })),
                        }],
                        type_args: None,
                    });
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swc_common::{Globals, GLOBALS};
    use swc_ecma_visit::VisitMutWith;

    use super::DynamicImport;
    use crate::ast::{build_js_ast, js_ast_to_code};

    #[test]
    fn test_dyanmic_import() {
        let code = r#"
import("./foo");
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
require.ensure([
    "./foo"
]).then(require.bind(require, "./foo"));

//# sourceMappingURL=index.js.map
            "#
            .trim()
        );
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context = Arc::new(Default::default());
        let mut ast = build_js_ast(path, origin, &context).unwrap();

        let globals = Globals::default();
        GLOBALS.set(&globals, || {
            let mut dyanmic_import = DynamicImport { context: &context };
            ast.ast.visit_mut_with(&mut dyanmic_import);
        });

        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
