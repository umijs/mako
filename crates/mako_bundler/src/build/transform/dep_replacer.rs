use std::collections::HashMap;
use swc_ecma_ast::{Callee, Expr, ExprOrSpread, Ident, Lit, Str};
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
                            let span = source.span;
                            // NOTE: JsWord 有缓存，直接设置 value 的方式在 dynamic type require 的情况下不会生效
                            *source = Str::from(replacement.clone());
                            // 保持原来的 span，不确定不加的话会不会导致 sourcemap 错误
                            (*source).span = span;
                        }
                    }
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}
