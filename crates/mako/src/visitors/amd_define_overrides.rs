use swc_core::common::{SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{as_folder, Fold, VisitMut};

pub struct AmdDefineOverrides {}

pub fn amd_define_overrides() -> impl VisitMut + Fold {
    as_folder(AmdDefineOverrides {})
}

impl VisitMut for AmdDefineOverrides {
    fn visit_mut_if_stmt(&mut self, node: &mut IfStmt) {
        if let box Expr::Bin(BinExpr {
            op: BinaryOp::LogicalAnd,
            left:
                box Expr::Bin(BinExpr {
                    op: BinaryOp::EqEqEq,
                    left:
                        box Expr::Unary(UnaryExpr {
                            op: UnaryOp::TypeOf,
                            arg: box Expr::Ident(Ident { sym, .. }),
                            ..
                        }),
                    ..
                }),
            ..
        }) = &node.test
        {
            if sym == "define" {
                node.test = Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    ctxt: SyntaxContext::empty(),
                    sym: "false".into(),
                    optional: false,
                }));
            }
        }
    }
}
