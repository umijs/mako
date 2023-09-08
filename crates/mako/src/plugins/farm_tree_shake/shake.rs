use anyhow::Result;

use crate::module::{ModuleAst, ModuleId, ModuleType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{TreeShakeModule, UsedExports};
use crate::plugins::farm_tree_shake::statement_graph::{ExportInfo, ImportInfo};
use crate::plugins::farm_tree_shake::{module, remove_useless_stmts, statement_graph};
use crate::tree_shaking_module::ModuleSystem;

/// tree shake useless modules and code, steps:
/// 1. topo sort the module_graph, the cyclic modules will be marked as side_effects
/// 2. generate tree_shake_modules based on the topo sorted modules
/// 3. traverse the tree_shake_modules
///   3.1 mark entry modules as side_effects
///   3.2 if module is commonjs, mark all imported modules as [UsedExports::All]
///   3.3 else if module is esm and the module has side effects, add imported identifiers to [UsedExports::Partial] of the imported modules
///   3.4 else if module is esm and the module has no side effects, analyze the used statement based on the statement graph
pub fn optimize_farm(module_graph: &mut ModuleGraph) -> Result<()> {
    // topo sort the module_graph, the cyclic modules will be marked as side_effects
    let (topo_sorted_modules, cyclic_modules) = { module_graph.toposort() };

    // mark cyclic modules as side_effects
    for chain in cyclic_modules {
        for module_id in chain {
            let module = module_graph.get_module_mut(&module_id).unwrap();
            module.side_effects = true;
        }
    }

    // mark entry modules as side_effects
    // entry side_effects marked in new stage

    let mut tree_shake_modules_ids = vec![];
    let mut tree_shake_modules_map = std::collections::HashMap::new();

    for module_id in topo_sorted_modules {
        let module = module_graph.get_module(&module_id).unwrap();

        let module_type = module.get_module_type();

        // skip non script modules and external modules
        if module_type != ModuleType::Script || module.is_external() {
            if module_type != ModuleType::Script && !module.is_external() {
                // mark all non script modules' script dependencies as side_effects
                for dep_id in module_graph.dependence_module_ids(&module_id) {
                    let dep_module = module_graph.get_module_mut(&dep_id).unwrap();

                    let dep_module_type = dep_module.get_module_type();

                    if dep_module_type != ModuleType::Script {
                        continue;
                    }

                    dep_module.side_effects = true;
                }
            }

            continue;
        };

        let tree_shake_module = TreeShakeModule::new(module);
        tree_shake_modules_ids.push(tree_shake_module.module_id.clone());
        tree_shake_modules_map.insert(tree_shake_module.module_id.clone(), tree_shake_module);
    }

    let mut modules_to_remove = vec![];

    // traverse the tree_shake_modules
    for tree_shake_module_id in tree_shake_modules_ids {
        let tree_shake_module = tree_shake_modules_map
            .get_mut(&tree_shake_module_id)
            .unwrap();

        // if module is not esm, mark all imported modules as [UsedExports::All]
        if !matches!(tree_shake_module.module_system, ModuleSystem::ESModule) {
            for (dep_id, _) in module_graph.get_dependencies(&tree_shake_module_id) {
                let dep_tree_shake_module = tree_shake_modules_map.get_mut(dep_id);

                if let Some(dep_tree_shake_module) = dep_tree_shake_module {
                    dep_tree_shake_module.used_exports = UsedExports::All;
                }
            }
        } else {
            // if module is esm and the module has side effects, add imported identifiers to [UsedExports::Partial] of the imported modules
            if tree_shake_module.side_effects {
                let imports = tree_shake_module.imports();
                let exports = tree_shake_module.exports();

                for import_info in &imports {
                    add_used_exports_by_import_info(
                        &mut tree_shake_modules_map,
                        &*module_graph,
                        &tree_shake_module_id,
                        import_info,
                    );
                }

                for export_info in &exports {
                    add_used_exports_by_export_info(
                        &mut tree_shake_modules_map,
                        &*module_graph,
                        &tree_shake_module_id,
                        export_info,
                    );
                }
            } else {
                let tree_shake_module = tree_shake_modules_map
                    .get_mut(&tree_shake_module_id)
                    .unwrap();

                if tree_shake_module.used_exports.is_empty() {
                    // if the module's used_exports is empty, means this module is not used and should be removed
                    modules_to_remove.push(tree_shake_module_id.clone());
                    continue;
                }

                let module = module_graph
                    .get_module_mut(&tree_shake_module.module_id)
                    .unwrap();
                let ast = &mut module.info.as_mut().unwrap().ast;

                if let ModuleAst::Script(swc_module) = ast {
                    // remove useless statements and useless imports/exports identifiers, then all preserved import info and export info will be added to the used_exports.
                    let (used_imports, used_exports_from) =
                        remove_useless_stmts::remove_useless_stmts(
                            tree_shake_module,
                            &mut swc_module.ast,
                        );

                    for import_info in used_imports {
                        add_used_exports_by_import_info(
                            &mut tree_shake_modules_map,
                            &*module_graph,
                            &tree_shake_module_id,
                            &import_info,
                        );
                    }

                    for export_info in used_exports_from {
                        add_used_exports_by_export_info(
                            &mut tree_shake_modules_map,
                            &*module_graph,
                            &tree_shake_module_id,
                            &export_info,
                        );
                    }
                }
            }
        }

        // add all dynamic imported dependencies as [UsedExports::All]
        for (dep, edge) in module_graph.get_dependencies(&tree_shake_module_id) {
            if matches!(edge.resolve_type, ResolveType::DynamicImport) {
                let tree_shake_module = tree_shake_modules_map.get_mut(dep).unwrap();
                tree_shake_module.side_effects = true;
                tree_shake_module.used_exports = UsedExports::All;
            }
        }
    }

    // remove the unused modules
    for module_id in modules_to_remove {
        module_graph.remove_module(&module_id);
    }

    Ok(())
}

// Add all imported to used_exports
fn add_used_exports_by_import_info(
    tree_shake_modules_map: &mut std::collections::HashMap<ModuleId, TreeShakeModule>,
    module_graph: &ModuleGraph,
    tree_shake_module_id: &ModuleId,
    import_info: &ImportInfo,
) {
    let imported_module_id =
        module_graph.get_dependency_module_by_source(tree_shake_module_id, &import_info.source);
    let imported_module = module_graph.get_module(imported_module_id).unwrap();

    let info = imported_module.info.as_ref().unwrap();

    let is_js = matches!(info.ast, ModuleAst::Script(_));

    if info.external.is_some() || !is_js {
        return;
    }

    let imported_tree_shake_module = tree_shake_modules_map
        .get_mut(imported_module_id)
        .unwrap_or_else(|| {
            panic!("imported module not found: {:?}", imported_module_id);
        });

    if import_info.specifiers.is_empty() {
        imported_tree_shake_module.used_exports = UsedExports::All;
        imported_tree_shake_module.side_effects = true;
        return;
    }

    for sp in &import_info.specifiers {
        match sp {
            statement_graph::ImportSpecifierInfo::Namespace(_) => {
                imported_tree_shake_module.used_exports = UsedExports::All;
            }
            statement_graph::ImportSpecifierInfo::Named { local, imported } => {
                if let Some(ident) = imported {
                    if *ident == "default" {
                        imported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::Default);
                    } else {
                        imported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::SwcIdent(strip_context(ident)));
                    }
                } else {
                    imported_tree_shake_module
                        .used_exports
                        .add_used_export(&module::UsedIdent::SwcIdent(strip_context(local)));
                }
            }
            statement_graph::ImportSpecifierInfo::Default(_) => {
                imported_tree_shake_module
                    .used_exports
                    .add_used_export(&module::UsedIdent::Default);
            }
        }
    }
}

/// All all exported to used_exports
fn add_used_exports_by_export_info(
    tree_shake_modules_map: &mut std::collections::HashMap<ModuleId, TreeShakeModule>,
    module_graph: &ModuleGraph,
    tree_shake_module_id: &ModuleId,
    export_info: &ExportInfo,
) {
    if let Some(source) = &export_info.source {
        let exported_module_id =
            module_graph.get_dependency_module_by_source(tree_shake_module_id, source);
        let exported_module = module_graph.get_module(exported_module_id).unwrap();

        if exported_module.is_external() {
            return;
        };

        let exported_tree_shake_module =
            tree_shake_modules_map.get_mut(exported_module_id).unwrap();

        for sp in &export_info.specifiers {
            match sp {
                statement_graph::ExportSpecifierInfo::Namespace(_) => {
                    exported_tree_shake_module.used_exports = module::UsedExports::All;
                }
                statement_graph::ExportSpecifierInfo::Named { local, .. } => {
                    if local == &"default".to_string() {
                        exported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::Default);
                    } else {
                        exported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::SwcIdent(strip_context(local)));
                    }
                }
                statement_graph::ExportSpecifierInfo::Default => {
                    exported_tree_shake_module
                        .used_exports
                        .add_used_export(&module::UsedIdent::Default);
                }
                statement_graph::ExportSpecifierInfo::All(used_idents) => {
                    if let Some(used_idents) = used_idents {
                        for ident in used_idents {
                            if ident == "*" {
                                exported_tree_shake_module.used_exports = module::UsedExports::All;
                            } else {
                                exported_tree_shake_module
                                    .used_exports
                                    .add_used_export(&strip_context(ident));
                            }
                        }
                    } else {
                        exported_tree_shake_module.used_exports = module::UsedExports::All;
                    }
                }
            }
        }
    }
}

fn strip_context(ident: &str) -> String {
    let ident_split = ident.split('#').collect::<Vec<_>>();
    ident_split[0].to_string()
}
