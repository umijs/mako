use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{CondExpr, Expr};
use mako_core::swc_ecma_utils::member_expr;
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::plugins::javascript::is_import_meta_url;

pub struct MetaUrlReplacer {}

impl VisitMut for MetaUrlReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if is_import_meta_url(expr) {
            // Compatible with nested workers: self.document ? self.document.baseURI : self.location.href
            *expr = Expr::Cond(CondExpr {
                span: DUMMY_SP,
                test: member_expr!(DUMMY_SP, self.document),
                cons: member_expr!(DUMMY_SP, self.document.baseURI),
                alt: member_expr!(DUMMY_SP, self.location.href),
            });
        }

        expr.visit_mut_children_with(self);
    }
}
