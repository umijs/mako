use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrowExpr, BindingIdent, BlockStmt, BlockStmtOrExpr, CallExpr, Callee, Expr, ExprOrSpread,
    ExprStmt, Ident, ModuleItem, Pat, Stmt,
};
use swc_ecma_visit::VisitMut;

pub struct AsyncModule {
    pub top_level_await: bool,
}

impl VisitMut for AsyncModule {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        // wrap async module with `require._async(module, async (handleAsyncDeps, asyncResult) => { });`
        *module_items = vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "require._async".into(),
                    optional: false,
                }))),
                type_args: None,
                args: vec![
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: "module".into(),
                            optional: false,
                        })),
                    },
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Arrow(ArrowExpr {
                            is_async: true,
                            is_generator: false,
                            type_params: None,
                            return_type: None,
                            span: DUMMY_SP,
                            params: vec![
                                Pat::Ident(BindingIdent {
                                    id: Ident {
                                        span: DUMMY_SP,
                                        sym: "handleAsyncDeps".into(),
                                        optional: false,
                                    },
                                    type_ann: None,
                                }),
                                Pat::Ident(BindingIdent {
                                    id: Ident {
                                        span: DUMMY_SP,
                                        sym: "asyncResult".into(),
                                        optional: false,
                                    },
                                    type_ann: None,
                                }),
                            ],
                            body: Box::new(BlockStmtOrExpr::BlockStmt(BlockStmt {
                                span: DUMMY_SP,
                                stmts: module_items
                                    .iter()
                                    .map(|stmt| stmt.as_stmt().unwrap().clone())
                                    .collect(),
                            })),
                        })),
                    },
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: if self.top_level_await { "1" } else { "0" }.into(),
                            optional: false,
                        })),
                    },
                ],
            })),
        }))];
    }
}
