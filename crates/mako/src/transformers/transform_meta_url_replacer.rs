use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{BinExpr, BinaryOp, Expr};
use mako_core::swc_ecma_utils::member_expr;
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::plugins::javascript::is_import_meta_url;

pub struct MetaUrlReplacer {}

impl VisitMut for MetaUrlReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if is_import_meta_url(expr) {
            *expr = Expr::Bin(BinExpr {
                span: DUMMY_SP,
                op: BinaryOp::LogicalOr,
                left: member_expr!(DUMMY_SP, document.baseURI),
                right: member_expr!(DUMMY_SP, self.location.href),
            });
        }

        expr.visit_mut_children_with(self);
    }
}
