use mako_core::indexmap::IndexMap;
use mako_core::swc_common::collections::AHashSet;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    Expr, Id, Ident, ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier,
    Module, ModuleDecl, ModuleItem,
};
use mako_core::swc_ecma_utils::{collect_decls, quote_ident, quote_str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::config::Providers;

pub struct Provide {
    bindings: Lrc<AHashSet<Id>>,
    providers: Providers,
    var_decls: IndexMap<String, ModuleItem>,
}

impl Provide {
    pub fn new(providers: Providers) -> Self {
        Self {
            bindings: Default::default(),
            providers,
            var_decls: Default::default(),
        }
    }
}

impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.bindings = Lrc::new(collect_decls(&*module));
        module.visit_mut_children_with(self);
        module
            .body
            .splice(0..0, self.var_decls.iter().map(|(_, var)| var.clone()));
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = self.bindings.contains(&(sym.clone(), span.ctxt));
            let name = &sym.to_string();
            let provider = self.providers.get(name);
            if !has_binding && provider.is_some() {
                if let Some((from, key)) = provider {
                    let require_decl: ModuleItem = {
                        if key.is_empty() {
                            // eg: import process from 'process';
                            ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                                span: DUMMY_SP,
                                specifiers: vec![ImportSpecifier::Default(
                                    ImportDefaultSpecifier {
                                        span: DUMMY_SP,
                                        local: quote_ident!(*span, sym.clone()),
                                    },
                                )],
                                src: quote_str!(from.as_str()).into(),
                                type_only: false,
                                with: None,
                            }))
                        } else {
                            // eg: import { Buffer } from 'buffer';
                            ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                                span: DUMMY_SP,
                                specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                                    span: DUMMY_SP,
                                    local: quote_ident!(*span, sym.clone()),
                                    imported: if sym.to_string().eq(key) {
                                        None
                                    } else {
                                        Some(quote_ident!(key.as_str()).into())
                                    },
                                    is_type_only: false,
                                })],
                                src: quote_str!(from.as_str()).into(),
                                type_only: false,
                                with: None,
                            }))
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
