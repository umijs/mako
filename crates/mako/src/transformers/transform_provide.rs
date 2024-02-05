use mako_core::indexmap::IndexMap;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{Expr, Ident, MemberExpr, Module, ModuleItem, VarDeclKind};
use mako_core::swc_ecma_utils::{quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_core::common::Mark;

use crate::config::Providers;
pub struct Provide {
    unresolved_mark: Mark,
    providers: Providers,
    var_decls: IndexMap<String, ModuleItem>,
}
impl Provide {
    pub fn new(providers: Providers, unresolved_mark: Mark) -> Self {
        Self {
            unresolved_mark,
            providers,
            var_decls: Default::default(),
        }
    }
}
impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);
        module
            .body
            .splice(0..0, self.var_decls.iter().map(|(_, var)| var.clone()));
    }
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = span.ctxt.outer() != self.unresolved_mark;
            let name = &sym.to_string();
            let provider = self.providers.get(name);
            if !has_binding && provider.is_some() {
                if let Some((from, key)) = provider {
                    let require_decl: ModuleItem = {
                        if key.is_empty() {
                            // eg: const process = require('process');
                            quote_ident!("__mako_require__")
                                .as_call(DUMMY_SP, vec![quote_str!(from.as_str()).as_arg()])
                                .into_var_decl(
                                    VarDeclKind::Const,
                                    quote_ident!(*span, sym.clone()).into(),
                                )
                                .into()
                        } else {
                            // require("buffer")
                            let require_expr = quote_ident!("__mako_require__")
                                .as_call(DUMMY_SP, vec![quote_str!(from.as_str()).as_arg()]);

                            // eg const Buffer = require("buffer").Buffer;
                            Expr::Member(MemberExpr {
                                obj: require_expr.into(),
                                span: DUMMY_SP,
                                prop: quote_ident!(key.as_str()).into(),
                            })
                            .into_var_decl(
                                VarDeclKind::Const,
                                quote_ident!(*span, sym.clone()).into(),
                            )
                            .into()
                        }
                    };

                    self.var_decls.insert(name.to_string(), require_decl);
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use mako_core::collections::HashMap;
    use swc_core::common::Mark;

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
        let mut providers = HashMap::default();
        providers.insert("process".into(), ("process".into(), "".into()));
        providers.insert("Buffer".into(), ("buffer".into(), "Buffer".into()));
        let mut visitor = super::Provide::new(providers, Mark::root());
        crate::transformers::test_helper::transform_js_code(code, &mut visitor, &context)
    }
}
