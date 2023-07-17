use swc_common::util::take::Take;
use swc_ecma_ast::{Decl, ExportDecl, ModuleItem};
use swc_ecma_visit::{VisitMut, VisitMutWith};
use tracing::debug;

use crate::comments::Comments;
use crate::module::ModuleId;

pub struct UnusedStatementSweep<'a> {
    module_id: ModuleId,
    pub comments: &'a Comments,
    need_removed_module_item: bool,
    pub removed_item_count: usize,
}

impl<'a> UnusedStatementSweep<'a> {
    pub fn new(module_id: &ModuleId, comments: &'a Comments) -> Self {
        Self {
            module_id: module_id.clone(),
            comments,
            need_removed_module_item: false,
            removed_item_count: 0,
        }
    }
}

impl VisitMut for UnusedStatementSweep<'_> {
    fn visit_mut_module(&mut self, n: &mut swc_ecma_ast::Module) {
        if self.comments.has_unused_module(n.span) {
            debug!("remove module {}", self.module_id.id);
            n.take();
        } else {
            n.visit_mut_children_with(self);
        }
    }

    fn visit_mut_module_item(&mut self, decl: &mut ModuleItem) {
        self.need_removed_module_item = false;

        decl.visit_mut_children_with(self);

        if self.need_removed_module_item {
            debug!("remove module item: {:?}", decl);
            decl.take();
        }
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut swc_ecma_ast::ImportDecl) {
        let mut removed = vec![];
        for (index, specifier) in import_decl.specifiers.iter().enumerate() {
            match specifier {
                swc_ecma_ast::ImportSpecifier::Named(named_specifier) => {
                    if self.comments.has_unused(named_specifier.span) {
                        removed.push(index);
                    }
                }
                _ => {}
            }
        }
        removed.reverse();
        for index in removed {
            import_decl.specifiers.remove(index);
            self.removed_item_count += 1;
        }
        if import_decl.specifiers.is_empty() {
            self.need_removed_module_item = true;
        }
    }

    fn visit_mut_export_specifiers(&mut self, specifiers: &mut Vec<swc_ecma_ast::ExportSpecifier>) {
        let mut removed = vec![];
        for (index, specifier) in specifiers.iter().enumerate() {
            if let swc_ecma_ast::ExportSpecifier::Named(named_specifier) = specifier {
                if self.comments.has_unused(named_specifier.span) {
                    removed.push(index);
                }
            }
        }
        removed.reverse();
        for index in removed {
            specifiers.remove(index);
            self.removed_item_count += 1;
        }
        if specifiers.is_empty() {
            self.need_removed_module_item = true;
        }
    }

    fn visit_mut_export_default_decl(
        &mut self,
        default_decl: &mut swc_ecma_ast::ExportDefaultDecl,
    ) {
        if self.comments.has_unused(default_decl.span) {
            self.need_removed_module_item = true;
            self.removed_item_count += 1;
        }
    }

    fn visit_mut_export_decl(&mut self, export_decl: &mut ExportDecl) {
        match &mut export_decl.decl {
            Decl::Var(var_decl) => {
                self.remove_unused_decls(var_decl);
                if var_decl.decls.is_empty() {
                    self.need_removed_module_item = true;
                }
            }
            Decl::Class(class_decl) => {
                if self.comments.has_unused(class_decl.ident.span) {
                    self.need_removed_module_item = true;
                    self.removed_item_count += 1;
                }
            }
            Decl::Fn(fn_decl) => {
                if self.comments.has_unused(fn_decl.ident.span) {
                    self.need_removed_module_item = true;
                    self.removed_item_count += 1;
                }
            }
            _ => (),
        }
    }
}

impl UnusedStatementSweep<'_> {
    fn remove_unused_decls(&mut self, var_decl: &mut Box<swc_ecma_ast::VarDecl>) {
        let mut removed = vec![];
        for (index, decl) in var_decl.decls.iter().enumerate() {
            if self.comments.has_unused(decl.span) {
                removed.push(index);
            }
        }
        removed.reverse();
        for index in removed {
            var_decl.decls.remove(index);
            self.removed_item_count += 1;
        }
    }
}
