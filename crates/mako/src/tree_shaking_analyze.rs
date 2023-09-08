use std::collections::HashMap;

use crate::compiler::Compiler;
use crate::module::ModuleId;
use crate::statement::{ExportSpecifier, ExportStatement, ImportStatement};
use crate::tree_shaking_module::{TreeShakingModule, UsedExports, UsedIdent};

impl Compiler {
    pub fn analyze_export_statement(
        &self,
        tree_shake_modules_map: &mut HashMap<ModuleId, TreeShakingModule>,
        tree_shaking_module_id: &ModuleId,
        export_statement: ExportStatement,
    ) {
        let module_graph = self.context.module_graph.write().unwrap();
        let used_export: UsedExports = tree_shake_modules_map
            .get_mut(tree_shaking_module_id)
            .unwrap()
            .used_exports
            .clone();
        if let Some(source) = &export_statement.info.source {
            let exported_module_id =
                module_graph.get_dependency_module_by_source(tree_shaking_module_id, source);
            let exported_module = module_graph.get_module(exported_module_id).unwrap();

            if exported_module.is_external() {
                return;
            }

            let exported_tree_shaking_module =
                tree_shake_modules_map.get_mut(exported_module_id).unwrap();

            for specifier in &export_statement.info.specifiers {
                match specifier {
                    ExportSpecifier::All => {
                        // 把*透传进去
                        if let UsedExports::Partial(ref idents) = used_export {
                            for ident in idents {
                                exported_tree_shaking_module
                                    .used_exports
                                    .add_used_export(&ident);
                            }
                        } else {
                            exported_tree_shaking_module.used_exports = UsedExports::All;
                        }
                    }
                    ExportSpecifier::Named { exported, local } => {
                        let used_ident = if strip_context(local) == "default" {
                            UsedIdent::Default
                        } else {
                            UsedIdent::SwcIdent(strip_context(local))
                        };

                        // export { default as foo } from './foo'
                        if let Some(exported) = exported {
                            // 当前文件如果有被用到的变量，才把目标模块的 default export 标记为 used
                            if used_export.contains(&strip_context(exported)) {
                                exported_tree_shaking_module
                                    .used_exports
                                    .add_used_export(&used_ident);
                            }
                            continue;
                        }

                        // 其余情况
                        if used_export.contains(&strip_context(local)) {
                            exported_tree_shaking_module
                                .used_exports
                                .add_used_export(&used_ident);
                        }
                    }
                    ExportSpecifier::Default => {
                        unreachable!("Export default not supported on source")
                    }
                    ExportSpecifier::Namespace(_) => {
                        exported_tree_shaking_module.used_exports = UsedExports::All;
                    }
                }
            }
        }
    }

    pub fn analyze_import_statement(
        &self,
        tree_shake_modules_map: &mut HashMap<ModuleId, TreeShakingModule>,
        tree_shaking_module_id: &ModuleId,
        import_statement: ImportStatement,
    ) {
        let module_graph = self.context.module_graph.write().unwrap();
        let imported_module_id = module_graph
            .get_dependency_module_by_source(tree_shaking_module_id, &import_statement.info.source);
        let imported_module = module_graph.get_module(imported_module_id).unwrap();

        if imported_module.is_external() || !imported_module.get_module_type().is_script() {
            return;
        }

        let imported_tree_shaking_module = tree_shake_modules_map
            .get_mut(imported_module_id)
            .unwrap_or_else(|| {
                panic!("imported module not found: {:?}", imported_module_id);
            });

        if import_statement.is_self_executed {
            imported_tree_shaking_module.side_effects = true;
            imported_tree_shaking_module.used_exports = UsedExports::All;
        }

        for specifier in &import_statement.info.specifiers {
            match specifier {
                // FIXME: 后面可以处理下 * as foo -> foo.F 这种情况下的 tree shaking，现在暂时不处理
                crate::statement::ImportSpecifier::Namespace(_) => {
                    imported_tree_shaking_module.used_exports = UsedExports::All;
                }
                crate::statement::ImportSpecifier::Named { local, imported } => {
                    if let Some(ident) = imported {
                        if strip_context(ident) == "default" {
                            imported_tree_shaking_module
                                .used_exports
                                .add_used_export(&UsedIdent::Default)
                        } else {
                            imported_tree_shaking_module
                                .used_exports
                                .add_used_export(&UsedIdent::SwcIdent(strip_context(ident)))
                        }
                    } else {
                        imported_tree_shaking_module
                            .used_exports
                            .add_used_export(&UsedIdent::SwcIdent(strip_context(local)))
                    }
                }
                crate::statement::ImportSpecifier::Default(_) => imported_tree_shaking_module
                    .used_exports
                    .add_used_export(&UsedIdent::Default),
            }
        }
    }
}

pub fn strip_context(ident: &str) -> String {
    let ident_split = ident.split('#').collect::<Vec<_>>();
    ident_split[0].to_string()
}
