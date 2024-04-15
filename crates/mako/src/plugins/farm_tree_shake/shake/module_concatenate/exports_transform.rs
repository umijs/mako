use std::collections::{HashMap, HashSet};

use swc_core::ecma::ast::{
    Decl, ExportSpecifier, Ident, Module, ModuleDecl, ModuleExportName, ObjectPatProp, Pat,
};
use swc_core::ecma::visit::{Visit, VisitWith};

#[derive(Default)]
pub(super) struct PatDefineIdCollector {
    defined_idents: HashSet<String>,
}

impl Visit for PatDefineIdCollector {
    fn visit_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(bi) => {
                self.defined_idents.insert(bi.id.sym.to_string());
            }
            Pat::Array(array_pat) => {
                for elem in array_pat.elems.iter().flatten() {
                    self.visit_pat(elem);
                }
            }
            Pat::Rest(rest_pat) => {
                self.visit_pat(&rest_pat.arg);
            }
            Pat::Object(obj_pat) => {
                for prop in &obj_pat.props {
                    match prop {
                        ObjectPatProp::KeyValue(kv_prop) => {
                            self.visit_pat(&kv_prop.value);
                        }
                        ObjectPatProp::Assign(assign_prop) => {
                            self.defined_idents.insert(assign_prop.key.to_string());
                        }
                        ObjectPatProp::Rest(rest_prop) => {
                            self.visit_pat(&rest_prop.arg);
                        }
                    }
                }
            }
            Pat::Assign(assign_pat) => {
                self.visit_pat(&assign_pat.left);
            }
            Pat::Invalid(_) => {}
            Pat::Expr(_) => {}
        }
    }
}

#[derive(Default)]
pub(super) struct ExportsCollector {
    exports: HashSet<String>,
}

impl Visit for ExportsCollector {
    fn visit_module_decl(&mut self, n: &ModuleDecl) {
        match n {
            ModuleDecl::Import(_) => {}
            ModuleDecl::ExportDecl(export_decl) => match &export_decl.decl {
                Decl::Class(class_decl) => {
                    self.exports.insert(class_decl.ident.sym.to_string());
                }
                Decl::Fn(fn_decl) => {
                    self.exports.insert(fn_decl.ident.sym.to_string());
                }
                Decl::Var(var_decl) => {
                    for x in var_decl.decls.iter() {
                        let names = collect_defined_ident_in_pat(&x.name);
                        self.exports.extend(names);
                    }
                }
                Decl::Using(_using_decl) => {
                    // TODO: when necessary
                    // for var_decl in using_decl.decls.iter() {
                    //     let names = collect_defined_ident_in_pat(&var_decl.name);
                    //     self.exports.extend(names);
                    // }
                }
                Decl::TsInterface(_) => {}
                Decl::TsTypeAlias(_) => {}
                Decl::TsEnum(_) => {}
                Decl::TsModule(_) => {}
            },
            ModuleDecl::ExportNamed(export_named) => export_named.specifiers.iter().for_each(
                |export_specifier| match &export_specifier {
                    ExportSpecifier::Namespace(namespace) => {
                        if let Some(ident) = module_export_name_as_ident(&namespace.name) {
                            self.exports.insert(ident.sym.to_string());
                        }
                    }
                    ExportSpecifier::Default(export_default) => {
                        self.exports.insert(export_default.exported.sym.to_string());
                    }
                    ExportSpecifier::Named(named_export) => {
                        if let Some(exported) = &named_export.exported
                            && let Some(ident) = module_export_name_as_ident(exported)
                        {
                            self.exports.insert(ident.sym.to_string());
                        } else if let Some(ident) = module_export_name_as_ident(&named_export.orig)
                        {
                            self.exports.insert(ident.sym.to_string());
                        }
                    }
                },
            ),
            ModuleDecl::ExportDefaultDecl(_) | ModuleDecl::ExportDefaultExpr(_) => {
                self.exports.insert("default".into());
            }
            ModuleDecl::ExportAll(_) => {
                // not allowed in inner module
            }
            ModuleDecl::TsImportEquals(_) => {}
            ModuleDecl::TsExportAssignment(_) => {}
            ModuleDecl::TsNamespaceExport(_) => {}
        }
    }
}

pub(super) fn collect_exports_map(module: &Module) -> HashMap<String, String> {
    let mut e: ExportsCollector = Default::default();
    module.visit_with(&mut e);
    e.exports.iter().map(|x| (x.clone(), x.clone())).collect()
}

fn collect_defined_ident_in_pat(pat: &Pat) -> HashSet<String> {
    let mut c: PatDefineIdCollector = Default::default();
    pat.visit_with(&mut c);
    c.defined_idents
}

fn module_export_name_as_ident(module_export_name: &ModuleExportName) -> Option<&Ident> {
    match module_export_name {
        ModuleExportName::Ident(ident) => Some(ident),
        _ => None,
    }
}
