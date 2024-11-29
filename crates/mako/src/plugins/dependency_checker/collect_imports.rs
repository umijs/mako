use std::collections::{HashMap, HashSet};

use swc_core::ecma::ast::*;
use swc_core::ecma::visit::Visit;

pub struct CollectImports<'a> {
    pub imports_specifiers_with_source: &'a mut HashMap<String, HashSet<String>>,
}

impl<'a> Visit for CollectImports<'a> {
    fn visit_import_decl(&mut self, node: &ImportDecl) {
        let source = node.src.value.to_string();
        if self.imports_specifiers_with_source.get(&source).is_none() {
            self.imports_specifiers_with_source
                .insert(source.clone(), HashSet::new());
        }

        node.specifiers
            .iter()
            .for_each(|specifier| match specifier {
                ImportSpecifier::Named(named) => {
                    if let Some(ModuleExportName::Ident(ident)) = &named.imported {
                        self.imports_specifiers_with_source
                            .get_mut(&source)
                            .unwrap()
                            .insert(ident.sym.to_string());
                    } else {
                        self.imports_specifiers_with_source
                            .get_mut(&source)
                            .unwrap()
                            .insert(named.local.sym.to_string());
                    }
                }
                ImportSpecifier::Default(_) => {
                    self.imports_specifiers_with_source
                        .get_mut(&source)
                        .unwrap()
                        .insert("default".into());
                }
                _ => {}
            })
    }

    fn visit_named_export(&mut self, node: &NamedExport) {
        let source = node.src.clone().unwrap().value;
        if self
            .imports_specifiers_with_source
            .get(source.as_str())
            .is_none()
        {
            self.imports_specifiers_with_source
                .insert(source.to_string(), HashSet::new());
        }

        if node.src.is_some() {
            node.specifiers.iter().for_each(|specifier| {
                if let ExportSpecifier::Named(named) = specifier {
                    if let ModuleExportName::Ident(ident) = &named.orig {
                        self.imports_specifiers_with_source
                            .get_mut(source.as_str())
                            .unwrap()
                            .insert(ident.sym.to_string());
                    }
                }
            })
        }
    }
}
