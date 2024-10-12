use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::*;
use swc_core::ecma::utils::private_ident;
use swc_core::ecma::visit::VisitMut;

const DEFAULT_COMPONENT_NAME: &str = "Component$$";

pub struct DefaultExportNamer {}

impl DefaultExportNamer {
    pub fn new() -> Self {
        Self {}
    }
}

impl VisitMut for DefaultExportNamer {
    fn visit_mut_module_item(&mut self, item: &mut ModuleItem) {
        if let ModuleItem::ModuleDecl(module_decl) = item {
            match module_decl {
                // we need to transfer ExportDefaultExpr(arrow_expr) to equivalent
                // ExportDefaultDecl other than changing directly from ExportDefaultExpr
                // (arrow_expr) to ExportDefaultExpr(fn_expr) which can't be hygiene properly.
                ModuleDecl::ExportDefaultExpr(ExportDefaultExpr {
                    expr: box Expr::Arrow(arrow_expr),
                    ..
                }) => {
                    if arrow_expr.is_async || arrow_expr.is_generator {
                        return;
                    }
                    let ArrowExpr {
                        params,
                        body,
                        is_async,
                        is_generator,
                        return_type,
                        type_params,
                        span,
                        ctxt,
                        ..
                    } = arrow_expr.clone();
                    *item = ModuleDecl::ExportDefaultDecl(ExportDefaultDecl {
                        span: DUMMY_SP,
                        decl: DefaultDecl::Fn(FnExpr {
                            ident: Some(private_ident!(DEFAULT_COMPONENT_NAME)),
                            function: Function {
                                params: params
                                    .into_iter()
                                    .map(|pat| pat.into())
                                    .collect::<Vec<_>>(),
                                body: Some(match *body {
                                    BlockStmtOrExpr::BlockStmt(block_stmt) => block_stmt,
                                    BlockStmtOrExpr::Expr(expr) => BlockStmt {
                                        span,
                                        ctxt,
                                        stmts: vec![Stmt::Return(ReturnStmt {
                                            span,
                                            arg: Some(expr),
                                        })],
                                    },
                                }),
                                is_async,
                                is_generator,
                                span,
                                ctxt,
                                return_type,
                                type_params,
                                decorators: vec![],
                            }
                            .into(),
                        }),
                    })
                    .into();
                }
                ModuleDecl::ExportDefaultDecl(decl) => {
                    if let DefaultDecl::Fn(fn_expr) = &mut decl.decl {
                        if fn_expr.ident.is_none() {
                            fn_expr.ident = Some(private_ident!(DEFAULT_COMPONENT_NAME));
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::DefaultExportNamer;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(
            run(r#"export default function(){}"#),
            "export default function Component$$() {}"
        );
    }

    #[test]
    fn test_conflicts() {
        assert_eq!(
            run(r#"export default function(){} let Component$$ = 1; Component$$ +=1;"#),
            "export default function Component$$() {}\nlet Component$$1 = 1;\nComponent$$1 += 1;"
        );
        assert_eq!(
            run(r#"let Component$$ = 1;export default function(){} Component$$ += 1;"#),
            "let Component$$ = 1;\nexport default function Component$$1() {}\nComponent$$ += 1;"
        );
    }

    #[test]
    fn test_arrow_function() {
        assert_eq!(
            run(r#"export default ()=>{}"#),
            "export default function Component$$() {}"
        );
    }

    #[test]
    fn test_arrow_function_exclude_cases() {
        assert_eq!(
            run(r#"export default async ()=>{};"#),
            "export default async ()=>{};"
        );
    }

    #[test]
    fn test_arrow_function_conflict() {
        assert_eq!(
            run(r#"let Component$$=1;export default ()=>{};Component$$+=1;"#),
            "let Component$$ = 1;\nexport default function Component$$1() {}\nComponent$$ += 1;"
        );
        assert_eq!(
            run(r#"export default ()=>{};let Component$$=1;Component$$+=1;"#),
            "export default function Component$$() {}\nlet Component$$1 = 1;\nComponent$$1 += 1;"
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DefaultExportNamer::new();
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
