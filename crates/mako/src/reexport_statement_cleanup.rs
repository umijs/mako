use std::collections::HashSet;

use swc_ecma_ast::{ModuleDecl, ModuleItem};
use swc_ecma_visit::VisitMut;
use tracing::debug;

use crate::module::ModuleId;
use crate::statement::StatementId;
use crate::tree_shaking_module::{should_skip, TreeShakingModule, UsedReExportHashMap};

/**
 * 针对 reexport 的场景做 move target 太复杂了，取个巧，直接把其他语句都删掉
 */
pub struct ReexportStatementCleanup {
    module_id: ModuleId,
    reexport_statements: UsedReExportHashMap,
}

impl ReexportStatementCleanup {
    pub fn new(tree_shaking_module: &TreeShakingModule) -> Self {
        let reexport_statements = tree_shaking_module.get_used_re_exports();
        Self {
            module_id: tree_shaking_module.id.clone(),
            reexport_statements,
        }
    }
}

impl VisitMut for ReexportStatementCleanup {
    fn visit_mut_module(&mut self, module: &mut swc_ecma_ast::Module) {
        if !self.reexport_statements.is_all_re_export() {
            return;
        }

        let ids: HashSet<StatementId> = self.reexport_statements.get_all_used_statement_ids();

        if ids.is_empty() {
            return;
        }

        debug!(
            "reexport statement cleanup {:?} {:?}",
            &self.module_id, &ids
        );
        let mut removed_ids = vec![];
        for (id, statement) in module.body.iter().enumerate() {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = statement {
                if should_skip(&import_decl.src.value) {
                    continue;
                }
            }

            if !ids.contains(&id) {
                removed_ids.push(id);
            }
        }

        removed_ids.sort();
        removed_ids.reverse();
        for id in &removed_ids {
            module.body.remove(*id);
        }
    }
}
