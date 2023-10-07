use swc_ecma_ast::{ModuleDecl, ModuleItem};
use swc_ecma_visit::VisitMut;
use tracing::debug;

use crate::module::ModuleId;
use crate::tree_shaking::tree_shaking_module::{should_skip, UsedIdentHashMap};

pub struct UnusedStatementCleanup<'a> {
    module_id: ModuleId,
    used_export_statement: &'a UsedIdentHashMap,
}

impl<'a> UnusedStatementCleanup<'a> {
    pub fn new(module_id: &ModuleId, used_export_statement: &'a UsedIdentHashMap) -> Self {
        Self {
            module_id: module_id.clone(),
            used_export_statement,
        }
    }
}

impl VisitMut for UnusedStatementCleanup<'_> {
    fn visit_mut_module(&mut self, module: &mut swc_ecma_ast::Module) {
        if should_skip(&self.module_id.id) {
            return;
        }
        let mut removed_ids = vec![];
        for (id, statement) in module.body.iter().enumerate() {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = statement {
                if should_skip(&import_decl.src.value) {
                    continue;
                }
            }
            if !self.used_export_statement.has_stmt_by_id(&id) {
                removed_ids.push(id);
            }
        }

        removed_ids.sort();
        removed_ids.reverse();
        for id in &removed_ids {
            module.body.remove(*id);
        }

        debug!(
            "reexport statement cleanup {:?} {:?} {:?}",
            &self.module_id, &removed_ids, &self.used_export_statement
        );
    }
}
