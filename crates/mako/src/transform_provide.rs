use swc_common::collections::AHashSet;
use swc_common::sync::Lrc;
use swc_ecma_ast::{
    Expr, Id, Ident, ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier,
    Module, ModuleDecl, ModuleItem,
};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::config::Providers;

pub struct Provide {
    bindings: Lrc<AHashSet<Id>>,
    providers: Providers,
    imports: Vec<ModuleItem>,
}

impl Provide {
    pub fn new(providers: Providers) -> Self {
        Self {
            bindings: Default::default(),
            providers,
            imports: vec![],
        }
    }
}

impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.bindings = Lrc::new(collect_decls(&*module));
        module.visit_mut_children_with(self);

        for import in self.imports.clone().into_iter() {
            module.body.push(import);
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = self.bindings.contains(&(sym.clone(), span.ctxt));
            let _syms = sym.to_string();
            let provider = self.providers.get(&sym.to_string());
            if !has_binding {
                if let Some((from, key)) = provider {
                    let import_decl = {
                        if key.is_empty() {
                            // import process from 'process'
                            ImportDecl {
                                span: *span,
                                specifiers: vec![ImportSpecifier::Default(
                                    ImportDefaultSpecifier {
                                        span: *span,
                                        local: Ident {
                                            span: *span,
                                            sym: sym.clone(),
                                            optional: false,
                                        },
                                    },
                                )],
                                src: Box::new(from.to_string().into()),
                                type_only: false,
                                asserts: None,
                            }
                        } else {
                            // import { Buffer } from 'buffer'
                            ImportDecl {
                                span: *span,
                                specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                                    span: *span,
                                    local: Ident {
                                        span: *span,
                                        sym: key.to_string().into(),
                                        optional: false,
                                    },
                                    imported: None,
                                    is_type_only: false,
                                })],
                                src: Box::new(from.to_string().into()),
                                type_only: false,
                                asserts: None,
                            }
                        }
                    };
                    self.imports
                        .push(ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)));
                }
            }
        }

        expr.visit_mut_children_with(self);
    }
}
