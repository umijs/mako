use std::sync::Arc;

use mako_core::swc_common::{Mark, Span};
use mako_core::swc_ecma_ast::{CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::plugins::javascript::is_native_ident;

pub struct MakoRequire<'a> {
    pub unresolved_mark: Mark,
    pub ignored_idents: &'a mut Vec<Span>,
    pub context: &'a Arc<Context>,
}

impl MakoRequire<'_> {
    fn is_ignored_ident(&self, ident: &Ident) -> bool {
        self.ignored_idents
            .iter()
            .any(|i| i.hi == ident.span.hi && i.lo == ident.span.lo)
    }
}

impl VisitMut for MakoRequire<'_> {
    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        // collect ignored identities from config.ignores
        if let (
            Some(ExprOrSpread {
                expr: box Expr::Lit(Lit::Str(Str { value, .. })),
                ..
            }),
            Callee::Expr(box Expr::Ident(ident)),
        ) = (&call_expr.args.get(0), &call_expr.callee)
        {
            let src = value.to_string();

            if ident.sym == *"require"
                && is_native_ident(ident, &self.unresolved_mark)
                && self.context.config.ignores.iter().any(|i| i == &src)
            {
                self.ignored_idents.push(ident.span);
            }
        }

        call_expr.visit_mut_children_with(self);
    }

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        // replace native require to __mako_require__ except ignored identities
        if ident.sym == *"require"
            && is_native_ident(ident, &self.unresolved_mark)
            && !self.is_ignored_ident(ident)
        {
            // println!("----> {:?}", ident);
            *ident = Ident::new("__mako_require__".into(), ident.span);
        }
    }
}
