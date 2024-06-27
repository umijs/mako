use std::collections::HashSet;

use swc_core::ecma::ast::{
    Decl, ExportSpecifier, Ident, ModuleDecl, ModuleExportName, ObjectPatProp, Pat,
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
                            self.defined_idents.insert(assign_prop.key.sym.to_string());
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

#[cfg(test)]
mod tests {
    use maplit::hashset;
    use swc_core::ecma::visit::VisitWith;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn collect_default_export() {
        assert_eq!(
            extract_export("export default 1"),
            hashset! {"default".to_string()}
        );
    }

    #[test]
    fn export_named_fn() {
        assert_eq!(
            extract_export("export function fn(){}"),
            hashset! {"fn".to_string()}
        );
    }

    #[test]
    fn export_named_class() {
        assert_eq!(
            extract_export("export class C{}"),
            hashset! {"C".to_string()}
        );
    }

    #[test]
    fn export_names() {
        assert_eq!(
            extract_export("let a=1,b=2; export {a,b}"),
            hashset! {"a".to_string(), "b".to_string()}
        );
    }

    #[test]
    fn export_object_deconstruct() {
        assert_eq!(
            extract_export("let A= {a:1,b:2, c:3}; export const {a,b:x, ...z} = A"),
            hashset! {"a".to_string(), "x".to_string(), "z".to_string()}
        );
    }

    #[test]
    fn export_array_deconstruct() {
        assert_eq!(
            extract_export("let a= [1,2,3]; export const [x,y,...z] = a"),
            hashset! {"x".to_string(), "y".to_string(), "z".to_string()}
        );
    }

    #[test]
    fn export_var_decl_export() {
        assert_eq!(
            extract_export("export const a =1"),
            hashset! {"a".to_string()}
        );
    }

    fn extract_export(code: &str) -> HashSet<String> {
        let mut ast = TestUtils::gen_js_ast(code);
        let mut collectort = ExportsCollector::default();

        ast.ast.js_mut().ast.visit_with(&mut collectort);
        collectort.exports
    }
}
