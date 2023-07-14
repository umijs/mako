use swc_common::util::take::Take;
use swc_ecma_ast::{Decl, ExportDecl, ModuleItem};
use swc_ecma_visit::{VisitMut, VisitMutWith};
use tracing::debug;

use crate::comments::Comments;

pub struct UnusedStatementSweep<'a> {
    pub comments: &'a Comments,
    need_removed_module_item: bool,
    pub removed_item_count: usize,
}

impl<'a> UnusedStatementSweep<'a> {
    pub fn new(comments: &'a Comments) -> Self {
        Self {
            comments,
            need_removed_module_item: false,
            removed_item_count: 0,
        }
    }
}

impl VisitMut for UnusedStatementSweep<'_> {
    fn visit_mut_module_item(&mut self, decl: &mut ModuleItem) {
        self.need_removed_module_item = false;

        decl.visit_mut_children_with(self);

        if self.need_removed_module_item {
            debug!("remove module item: {:?}", decl);
            decl.take();
        }
    }
    // TODO: import specifiers removed

    fn visit_mut_export_specifiers(&mut self, specifiers: &mut Vec<swc_ecma_ast::ExportSpecifier>) {
        let mut removed = vec![];
        for (index, specifier) in specifiers.iter().enumerate() {
            if let swc_ecma_ast::ExportSpecifier::Named(named_specifier) = specifier {
                if self.comments.has_unused(named_specifier.span) {
                    removed.push(index);
                }
            }
        }
        for index in removed {
            specifiers.remove(index);
            self.removed_item_count += 1;
        }
        if specifiers.is_empty() {
            self.need_removed_module_item = true;
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
        for index in removed {
            var_decl.decls.remove(index);
            self.removed_item_count += 1;
        }
    }
}
