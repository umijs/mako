use swc_atoms::JsWord;
use swc_common::collections::AHashSet;
use swc_common::sync::Lrc;
use swc_common::DUMMY_SP;
use swc_ecma_ast::{CallExpr, Callee, Expr, ExprOrSpread, Id, Ident, Lit, MemberExpr, Module, Str};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::config::Providers;

pub struct Provide {
    bindings: Lrc<AHashSet<Id>>,
    providers: Providers,
}

impl Provide {
    pub fn new(providers: Providers) -> Self {
        Self {
            bindings: Default::default(),
            providers,
        }
    }
}

impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.bindings = Lrc::new(collect_decls(&*module));
        module.visit_mut_children_with(self);
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = self.bindings.contains(&(sym.clone(), span.ctxt));
            let provider = self.providers.get(&sym.to_string());
            if !has_binding {
                if let Some((from, key)) = provider {
                    // require("provider")
                    let new_expr = Expr::Call(CallExpr {
                        span: *span,
                        callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                            span: *span,
                            sym: "require".into(),
                            optional: false,
                        }))),
                        args: vec![ExprOrSpread {
                            spread: None,
                            expr: Box::new(Expr::Lit(Lit::Str(Str {
                                span: DUMMY_SP,
                                value: JsWord::from(from.clone()),
                                raw: None,
                            }))),
                        }],
                        type_args: None,
                    });
                    if !key.is_empty() {
                        // require("buffer").Buffer
                        let new_expr = Expr::Member(MemberExpr {
                            obj: Box::new(new_expr),
                            span: DUMMY_SP,
                            prop: swc_ecma_ast::MemberProp::Ident(Ident {
                                span: *span,
                                sym: JsWord::from(key.clone()),
                                optional: false,
                            }),
                        });
                        *expr = new_expr;
                    } else {
                        *expr = new_expr;
                    }
                }
            }
        }

        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn test_provide_normal() {
        // TODO: fix binding test problem
        crate::assert_display_snapshot!(transform(
            r#"
console.log(process);
console.log(process.env);
Buffer.from('foo');
function foo() {
    // let process = 1;
    // console.log(process);
    // let Buffer = 'b';
    // Buffer.from('foo');
}
            "#,
        ));
    }

    fn transform(code: &str) -> String {
        let context = std::sync::Arc::new(Default::default());
        let mut providers = HashMap::new();
        providers.insert("process".into(), ("process".into(), "".into()));
        providers.insert("Buffer".into(), ("buffer".into(), "Buffer".into()));
        let mut visitor = super::Provide::new(providers);
        crate::transformers::test_helper::transform_js_code(code, &mut visitor, &context)
    }
}
