use std::sync::Arc;

use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::{CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::plugins::javascript::is_native_ident;

pub struct MakoRequire<'a> {
    pub unresolved_mark: Mark,
    pub context: &'a Arc<Context>,
}

impl MakoRequire<'_> {
    fn try_to_replace_require(&mut self, ident: &mut Ident) {
        // replace native require to __mako_require__ except ignored identities
        if ident.sym == *"require" && is_native_ident(ident, &self.unresolved_mark) {
            *ident = Ident::new("__mako_require__".into(), ident.span);
        }
    }
}

impl VisitMut for MakoRequire<'_> {
    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        if let (
            Some(ExprOrSpread {
                expr: box Expr::Lit(Lit::Str(Str { value, .. })),
                ..
            }),
            Callee::Expr(box Expr::Ident(ident)),
        ) = (&call_expr.args.get(0), &mut call_expr.callee)
        {
            let src = value.to_string();

            // replace native require call expression to __mako_require__ except ignored identities
            if !self.context.config.ignores.iter().any(|i| i == &src) {
                self.try_to_replace_require(ident);
            }

            // skip visit callee to avoid replace ignored require by self.visit_mut_ident
            call_expr.args.visit_mut_children_with(self);
        } else {
            call_expr.visit_mut_children_with(self);
        }
    }

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        self.try_to_replace_require(ident);
    }
}
