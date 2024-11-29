use std::collections::HashSet;

use swc_core::ecma::ast::*;
use swc_core::ecma::visit::Visit;

pub struct CollectExports<'a> {
    pub specifiers: &'a mut HashSet<String>,
    pub exports_star_sources: &'a mut Vec<String>,
}

impl<'a> Visit for CollectExports<'a> {
    fn visit_module_decl(&mut self, node: &ModuleDecl) {
        match &node {
            // export const a = 1
            ModuleDecl::ExportDecl(ExportDecl { decl, .. }) => match decl {
                Decl::Fn(FnDecl { ident, .. }) => {
                    self.specifiers.remove(&ident.sym.to_string());
                }
                Decl::Class(ClassDecl { ident, .. }) => {
                    self.specifiers.remove(&ident.sym.to_string());
                }
                Decl::Var(box VarDecl { decls, .. }) => decls.iter().for_each(|decl| {
                    if let Pat::Ident(ident) = &decl.name {
                        self.specifiers.remove(&ident.sym.to_string());
                    }
                }),
                _ => {}
            },
            // export default function
            ModuleDecl::ExportDefaultDecl(_) => {
                self.specifiers.remove(&"default".to_string());
            }
            // export default 1
            ModuleDecl::ExportDefaultExpr(_) => {
                self.specifiers.remove(&"default".to_string());
            }
            // export * from 'b'
            ModuleDecl::ExportAll(all) => {
                let source = all.src.value.to_string();
                self.exports_star_sources.push(source);
            }
            // export {a, b} || export {default as c} from 'd'
            ModuleDecl::ExportNamed(named) => {
                named.specifiers.iter().for_each(|specifier| {
                    match &specifier {
                        ExportSpecifier::Named(named) => {
                            if let Some(ModuleExportName::Ident(ident)) = &named.exported {
                                self.specifiers.remove(&ident.sym.to_string());
                            }
                        }
                        ExportSpecifier::Namespace(name_spacing) => {
                            if let ModuleExportName::Ident(ident) = &name_spacing.name {
                                self.specifiers.remove(&ident.sym.to_string());
                            }
                        }
                        _ => {
                            //@todo what is ExportDefaultSpecifier?
                        }
                    }
                })
            }
            _ => {}
        }
    }
}
