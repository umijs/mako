use std::collections::HashMap;

use swc_ecma_ast::{Callee, Expr, ExprOrSpread, Ident, Lit};
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct DepReplacer {
    pub dep_map: HashMap<String, String>,
}

impl VisitMut for DepReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if let Callee::Expr(box Expr::Ident(Ident { sym, .. })) = &call_expr.callee {
                if sym == "require" {
                    if let ExprOrSpread {
                        expr: box Expr::Lit(Lit::Str(ref mut source)),
                        ..
                    } = &mut call_expr.args[0]
                    {
                        if let Some(replacement) = self.dep_map.get(&source.value.to_string()) {
                            source.value = replacement.clone().into();
                        }
                    }
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}
