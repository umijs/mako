#[cfg(test)]
mod tests;

use swc_core::ecma::ast::{
    DefaultDecl, Id, Ident, ImportDecl, ImportSpecifier, Module, ModuleItem,
};

#[derive(Debug, Clone)]
pub(super) enum VarLink {
    Direct(Id),
    InDirect(Symbol, String),
    All(String, usize),
}

#[derive(Debug, Clone)]
pub(super) enum Symbol {
    Default,
    Namespace,
    Var(Ident),
}

impl Symbol {
    pub fn as_id(&self) -> Option<Id> {
        match self {
            Symbol::Default => None,
            Symbol::Namespace => None,
            Symbol::Var(ident) => Some(ident.to_id()),
        }
    }

    pub fn to_field(&self) -> Option<JsWord> {
        match self {
            Symbol::Default => js_word!("default").into(),
            Symbol::Namespace => None,
            Symbol::Var(ident) => Some(ident.sym.clone()),
        }
    }
}

use std::collections::{HashMap, HashSet};

use swc_core::base::atoms::{js_word, JsWord};
use swc_core::common::SyntaxContext;
use swc_core::ecma::ast::{
    Decl, ExportSpecifier, ModuleDecl, ModuleExportName, ObjectPatProp, Pat,
};
use swc_core::ecma::utils::quote_ident;
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::DUMMY_CTXT;

#[derive(Default)]
pub(super) struct PatDefineIdCollector {
    defined_idents: HashSet<Ident>,
}

impl Visit for PatDefineIdCollector {
    fn visit_pat(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(bi) => {
                self.defined_idents.insert(bi.id.clone());
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
                            self.defined_idents.insert(assign_prop.key.clone().into());
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

#[derive(Default, Debug)]
pub(super) struct ModuleDeclMapCollector {
    current_source: Option<String>,
    current_stmt_id: usize,
    pub import_map: HashMap<Id, VarLink>,
    pub export_map: HashMap<Id, VarLink>,
    default_binding_name: String,
}

impl ModuleDeclMapCollector {
    pub fn new(default_binding_name: String) -> Self {
        Self {
            default_binding_name,
            ..Default::default()
        }
    }
    fn simplify_exports(&mut self) {
        self.export_map.iter_mut().for_each(|(_ident, link)| {
            if let VarLink::Direct(exported) = link {
                if let Some(import_link) = self.import_map.get(exported) {
                    *link = import_link.clone();
                }
            }
        })
    }

    fn insert_export_var_link(&mut self, ident: Ident, symbol: Symbol) {
        let symbol = match symbol {
            Symbol::Var(ident) if "default".eq(&ident.sym) => Symbol::Default,
            _ => symbol,
        };

        if let Some(source) = &self.current_source {
            self.export_map
                .insert(ident.to_id(), VarLink::InDirect(symbol, source.clone()));
        } else if let Some(id) = symbol.as_id() {
            self.export_map.insert(ident.to_id(), VarLink::Direct(id));
        } else {
            panic!(
                "export without source can only export ident but got {:?}",
                symbol
            );
        }
    }
}

impl Visit for ModuleDeclMapCollector {
    fn visit_module(&mut self, n: &Module) {
        n.visit_children_with(self);
        self.simplify_exports();
    }
    fn visit_module_items(&mut self, n: &[ModuleItem]) {
        n.iter().enumerate().for_each(|(i, item)| {
            self.current_stmt_id = i;
            item.visit_with(self);
        });
    }

    fn visit_import_decl(&mut self, import_decl: &ImportDecl) {
        let current_source = self.current_source.as_ref().unwrap();

        import_decl.specifiers.iter().for_each(|spec| match spec {
            ImportSpecifier::Named(named_spec) => {
                let local = named_spec.local.clone();
                let imported = named_spec.imported.as_ref().map_or_else(
                    || local.clone(),
                    |imported| match imported {
                        ModuleExportName::Ident(imported_ident) => imported_ident.clone(),
                        ModuleExportName::Str(_) => {
                            unimplemented!("import as string not supported now")
                        }
                    },
                );

                self.import_map.insert(
                    local.to_id(),
                    VarLink::InDirect(Symbol::Var(imported), current_source.clone()),
                );
            }
            ImportSpecifier::Default(default_ident) => {
                self.import_map.insert(
                    default_ident.local.to_id(),
                    VarLink::InDirect(Symbol::Default, current_source.clone()),
                );
            }
            ImportSpecifier::Namespace(import_star) => {
                self.import_map.insert(
                    import_star.local.to_id(),
                    VarLink::InDirect(Symbol::Namespace, current_source.clone()),
                );
            }
        })
    }

    fn visit_module_decl(&mut self, n: &ModuleDecl) {
        match n {
            ModuleDecl::Import(import) => {
                self.current_source = Some(import.src.value.to_string());
                n.visit_children_with(self);
                self.current_source = None;
            }
            ModuleDecl::ExportDecl(export_decl) => match &export_decl.decl {
                Decl::Class(class_decl) => {
                    let class_ident = class_decl.ident.clone();
                    self.export_map
                        .insert(class_ident.to_id(), VarLink::Direct(class_ident.to_id()));
                }
                Decl::Fn(fn_decl) => {
                    let fn_ident = fn_decl.ident.clone();
                    self.export_map
                        .insert(fn_ident.to_id(), VarLink::Direct(fn_ident.to_id()));
                }
                Decl::Var(var_decl) => {
                    for x in var_decl.decls.iter() {
                        let idents = collect_defined_ident_in_pat(&x.name);

                        for ident in idents {
                            self.export_map
                                .insert(ident.to_id(), VarLink::Direct(ident.to_id()));
                        }
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
            ModuleDecl::ExportNamed(export_named) => {
                self.current_source = export_named.src.as_ref().map(|x| x.value.to_string());
                export_named.specifiers.iter().for_each(
                    |export_specifier| match &export_specifier {
                        ExportSpecifier::Namespace(namespace) => {
                            if let Some(ident) = module_export_name_as_ident(&namespace.name) {
                                self.insert_export_var_link(ident.clone(), Symbol::Namespace)
                            }
                        }
                        ExportSpecifier::Default(export_default) => {
                            self.insert_export_var_link(
                                export_default.exported.clone(),
                                Symbol::Default,
                            );
                        }
                        ExportSpecifier::Named(named_export) => {
                            if let Some(exported) = &named_export.exported
                                && let Some(ident) = module_export_name_as_ident(exported)
                            {
                                if let Some(orig_ident) =
                                    module_export_name_as_ident(&named_export.orig)
                                {
                                    self.insert_export_var_link(
                                        ident.clone(),
                                        Symbol::Var(orig_ident.clone()),
                                    );
                                }
                            } else if let Some(ident) =
                                module_export_name_as_ident(&named_export.orig)
                            {
                                self.insert_export_var_link(
                                    ident.clone(),
                                    Symbol::Var(ident.clone()),
                                );
                            }
                        }
                    },
                );
                self.current_source = None;
            }
            ModuleDecl::ExportDefaultDecl(export_default_decl) => {
                let fallback_id = (
                    self.default_binding_name.clone().into(),
                    SyntaxContext::empty(),
                );

                let default_id = match &export_default_decl.decl {
                    DefaultDecl::Class(class_expr) => class_expr
                        .ident
                        .as_ref()
                        .map_or(fallback_id, |ident: &Ident| ident.to_id()),
                    DefaultDecl::Fn(fn_expr) => fn_expr
                        .ident
                        .as_ref()
                        .map_or(fallback_id, |ident: &Ident| ident.to_id()),
                    DefaultDecl::TsInterfaceDecl(_) => {
                        unreachable!()
                    }
                };

                self.export_map.insert(
                    quote_ident!(DUMMY_CTXT, "default").to_id(),
                    VarLink::Direct(default_id),
                );
            }
            ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                let id = match export_default_expr.expr.as_ident() {
                    Some(ident) => ident.to_id(),
                    None => (
                        self.default_binding_name.clone().into(),
                        SyntaxContext::empty(),
                    ),
                };

                self.export_map.insert(
                    quote_ident!(DUMMY_CTXT, "default").to_id(),
                    VarLink::Direct(id),
                );
            }
            ModuleDecl::ExportAll(export_all) => {
                self.export_map.insert(
                    quote_ident!(DUMMY_CTXT, format!("*:{}", self.current_stmt_id)).to_id(),
                    VarLink::All(export_all.src.value.to_string(), self.current_stmt_id),
                );
            }
            ModuleDecl::TsImportEquals(_) => {}
            ModuleDecl::TsExportAssignment(_) => {}
            ModuleDecl::TsNamespaceExport(_) => {}
        }
    }
}

fn collect_defined_ident_in_pat(pat: &Pat) -> HashSet<Ident> {
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
