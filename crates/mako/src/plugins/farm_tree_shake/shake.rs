use std::cell::RefCell;
use std::ops::DerefMut;

use anyhow::Result;

use crate::module::{ModuleAst, ModuleId, ModuleType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{TreeShakeModule, UsedExports};
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo, ImportInfo,
};
use crate::plugins::farm_tree_shake::{module, remove_useless_stmts, statement_graph};
use crate::tree_shaking::tree_shaking_module::ModuleSystem;

/// tree shake useless modules and code, steps:
/// 1. topo sort the module_graph, the cyclic modules treat as no side_effects
/// 2. generate tree_shake_modules based on the topo sorted order
/// 3. reserve traverse the tree_shake_modules to fill all_exports
/// 3. traverse the tree_shake_modules
///   3.2 if module is commonjs, mark all imported modules as [UsedExports::All] by calling `.use_all()`
///   3.3 else if module is esm and the module has side effects, add imported identifiers to [UsedExports::Partial] of the imported modules
///     if add imported identifiers to previous modules, traverse smallest index tree_shake_modules again
///   3.4 else if module is esm and the module has no side effects, analyze the used statement based on the statement graph
///     if add imported identifiers to previous modules, traverse smallest index tree_shake_modules again
/// 4. remove used module and update tree-shaked AST into module graph
pub fn optimize_farm(module_graph: &mut ModuleGraph) -> Result<()> {
    let (topo_sorted_modules, _cyclic_modules) = {
        mako_core::mako_profile_scope!("tree shake toposort");
        module_graph.toposort()
    };

    let mut tree_shake_modules_ids = vec![];
    let mut tree_shake_modules_map = std::collections::HashMap::new();

    let mut order: usize = 0;
    for module_id in topo_sorted_modules.iter() {
        let module = module_graph.get_module(module_id).unwrap();

        let module_type = module.get_module_type();

        // skip non script modules and external modules
        if module_type != ModuleType::Script || module.is_external() {
            if module_type != ModuleType::Script && !module.is_external() {
                // mark all non script modules' script dependencies as side_effects
                for dep_id in module_graph.dependence_module_ids(module_id) {
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

        let tree_shake_module = TreeShakeModule::new(module, order);
        order += 1;
        tree_shake_modules_ids.push(tree_shake_module.module_id.clone());
        tree_shake_modules_map.insert(
            tree_shake_module.module_id.clone(),
            RefCell::new(tree_shake_module),
        );
    }

    // fill tree shake module all exported ident in reversed opo-sort order
    for module_id in tree_shake_modules_ids.iter().rev() {
        let mut tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();

        for exp_info in tsm.exports() {
            if let Some(source) = exp_info.source {
                for sp_info in exp_info.specifiers {
                    match sp_info {
                        ExportSpecifierInfo::All(_) => {
                            let dependent_id =
                                module_graph.get_dependency_module_by_source(module_id, &source);

                            if let Some(rc) = tree_shake_modules_map.get(dependent_id) {
                                let dependence_module = rc.borrow();

                                let to_extend = dependence_module.all_exports.clone();

                                tsm.stmt_graph.stmt_mut(&exp_info.stmt_id).export_info =
                                    Some(ExportInfo {
                                        source: Some(source.clone()),
                                        specifiers: vec![ExportSpecifierInfo::All(Some(
                                            to_extend
                                                .clone()
                                                .into_iter()
                                                .filter(|id| id != "default")
                                                .collect::<Vec<_>>(),
                                        ))],
                                        stmt_id: exp_info.stmt_id,
                                    });

                                tsm.all_exports.extend(to_extend.into_iter());
                            }
                        }
                        _ => {
                            tsm.all_exports.extend(sp_info.to_idents());
                        }
                    }
                }
            } else {
                for exp_sp in exp_info.specifiers {
                    let idents = exp_sp.to_idents();
                    tsm.all_exports.extend(idents);
                }
            }
        }
    }

    // traverse the tree_shake_modules
    let mut current_index: usize = 0;
    let len = tree_shake_modules_ids.len();
    while current_index < len {
        let mut next_index = current_index + 1;

        let tree_shake_module_id = &tree_shake_modules_ids[current_index];

        let mut tree_shake_module = tree_shake_modules_map
            .get(tree_shake_module_id)
            .unwrap()
            .borrow_mut();

        // if module is not esm, mark all imported modules as [UsedExports::All]
        if !matches!(tree_shake_module.module_system, ModuleSystem::ESModule) {
            for (dep_id, _) in module_graph.get_dependencies(tree_shake_module_id) {
                if let Some(ref_cell) = tree_shake_modules_map.get(dep_id) {
                    let mut dep_module = ref_cell.borrow_mut();

                    if dep_module.used_exports.use_all() && dep_module.topo_order < next_index {
                        next_index = dep_module.topo_order;
                    }
                }
            }
        } else {
            // if module is esm and the module has side effects, add imported identifiers to [UsedExports::Partial] of the imported modules
            if tree_shake_module.side_effects {
                let imports = tree_shake_module.imports();
                let exports = tree_shake_module.exports();

                for import_info in &imports {
                    if let Some(order) = add_used_exports_by_import_info(
                        &tree_shake_modules_map,
                        &*module_graph,
                        tree_shake_module_id,
                        import_info,
                    ) {
                        if order < next_index {
                            next_index = order;
                        }
                    }
                }

                for export_info in &exports {
                    if let Some(order) = add_used_exports_by_export_info(
                        &tree_shake_modules_map,
                        &*module_graph,
                        tree_shake_module_id,
                        tree_shake_module.side_effects,
                        export_info,
                    ) {
                        if order < next_index {
                            next_index = order;
                        }
                    }
                }
            } else {
                if tree_shake_module.used_exports.is_empty() {
                    // if the module's used_exports is empty, means this module is not used and will be removed
                    current_index = next_index;

                    continue;
                }

                let module = module_graph
                    .get_module_mut(&tree_shake_module.module_id)
                    .unwrap();
                let ast = &mut module.info.as_mut().unwrap().ast;

                if let ModuleAst::Script(swc_module) = ast {
                    // remove useless statements and useless imports/exports identifiers, then all preserved import info and export info will be added to the used_exports.

                    let mut shadow = swc_module.ast.clone();

                    let (used_imports, used_exports_from) =
                        remove_useless_stmts::remove_useless_stmts(
                            tree_shake_module.deref_mut(),
                            &mut shadow,
                        );

                    tree_shake_module.updated_ast = Some(shadow);

                    for import_info in used_imports {
                        if let Some(order) = add_used_exports_by_import_info(
                            &tree_shake_modules_map,
                            &*module_graph,
                            tree_shake_module_id,
                            &import_info,
                        ) {
                            if next_index > order {
                                next_index = order;
                            }
                        }
                    }

                    for export_info in used_exports_from {
                        if let Some(order) = add_used_exports_by_export_info(
                            &tree_shake_modules_map,
                            &*module_graph,
                            tree_shake_module_id,
                            tree_shake_module.side_effects,
                            &export_info,
                        ) {
                            if next_index > order {
                                next_index = order;
                            }
                        }
                    }
                }
            }
        }

        // add all dynamic imported dependencies as [UsedExports::All]
        for (dep, edge) in module_graph.get_dependencies(tree_shake_module_id) {
            match edge.resolve_type {
                ResolveType::DynamicImport => {
                    let mut tree_shake_module =
                        tree_shake_modules_map.get(dep).unwrap().borrow_mut();
                    if tree_shake_module.used_exports.use_all()
                        && tree_shake_module.topo_order < next_index
                    {
                        next_index = tree_shake_module.topo_order;
                    }

                    tree_shake_module.side_effects = true;
                }
                ResolveType::Require => {
                    if let Some(ref_cell) = tree_shake_modules_map.get(dep) {
                        let mut tree_shake_module = ref_cell.borrow_mut();

                        if tree_shake_module.used_exports.use_all()
                            && tree_shake_module.topo_order < next_index
                        {
                            next_index = tree_shake_module.topo_order;
                        }
                    }
                }
                _ => {}
            }
        }

        current_index = next_index;
    }

    for (module_id, tsm) in tree_shake_modules_map {
        let tsm = tsm.borrow();

        if tsm.used_exports.is_empty() {
            module_graph.remove_module(&module_id);
        } else if let Some(swc_module) = &tsm.updated_ast {
            module_graph
                .get_module_mut(&module_id)
                .unwrap()
                .info
                .as_mut()
                .unwrap()
                .ast
                .as_script_mut()
                .body = swc_module.body.clone();
        }
    }

    Ok(())
}

// Add all imported to used_exports
// returns (added, imported_module_topo_order)
fn add_used_exports_by_import_info(
    tree_shake_modules_map: &std::collections::HashMap<ModuleId, RefCell<TreeShakeModule>>,
    module_graph: &ModuleGraph,
    tree_shake_module_id: &ModuleId,
    import_info: &ImportInfo,
) -> Option<usize> {
    let imported_module_id =
        module_graph.get_dependency_module_by_source(tree_shake_module_id, &import_info.source);
    let imported_module = module_graph.get_module(imported_module_id).unwrap();

    let info = imported_module.info.as_ref().unwrap();

    let is_js = matches!(info.ast, ModuleAst::Script(_));

    if info.external.is_some() || !is_js {
        return None;
    }

    let mut imported_tree_shake_module = tree_shake_modules_map
        .get(imported_module_id)
        .unwrap_or_else(|| {
            panic!("imported module not found: {:?}", imported_module_id);
        })
        .borrow_mut();

    if import_info.specifiers.is_empty() {
        imported_tree_shake_module.used_exports.use_all();
        imported_tree_shake_module.side_effects = true;
        return Some(imported_tree_shake_module.topo_order);
    }

    let mut added = false;

    for sp in &import_info.specifiers {
        match sp {
            statement_graph::ImportSpecifierInfo::Namespace(_) => {
                imported_tree_shake_module.used_exports = UsedExports::All;
            }
            statement_graph::ImportSpecifierInfo::Named { local, imported } => {
                if let Some(ident) = imported {
                    if *ident == "default" {
                        added |= imported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::Default);
                    } else {
                        added |= imported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::SwcIdent(strip_context(ident)));
                    }
                } else {
                    added |= imported_tree_shake_module
                        .used_exports
                        .add_used_export(&module::UsedIdent::SwcIdent(strip_context(local)));
                }
            }
            statement_graph::ImportSpecifierInfo::Default(_) => {
                added |= imported_tree_shake_module
                    .used_exports
                    .add_used_export(&module::UsedIdent::Default);
            }
        }
    }

    if added {
        Some(imported_tree_shake_module.topo_order)
    } else {
        None
    }
}

/// All all exported to used_exports
fn add_used_exports_by_export_info(
    tree_shake_modules_map: &std::collections::HashMap<ModuleId, RefCell<TreeShakeModule>>,
    module_graph: &ModuleGraph,
    tree_shake_module_id: &ModuleId,
    has_side_effects: bool,
    export_info: &ExportInfo,
) -> Option<usize> {
    if let Some(source) = &export_info.source {
        let exported_module_id =
            module_graph.get_dependency_module_by_source(tree_shake_module_id, source);
        let exported_module = module_graph.get_module(exported_module_id).unwrap();

        if exported_module.is_external() {
            return None;
        };

        let mut exported_tree_shake_module = tree_shake_modules_map
            .get(exported_module_id)
            .unwrap()
            .borrow_mut();

        let mut added = false;

        for sp in &export_info.specifiers {
            match sp {
                statement_graph::ExportSpecifierInfo::Namespace(_) => {
                    added |= exported_tree_shake_module.used_exports.use_all();
                }
                statement_graph::ExportSpecifierInfo::Named { local, .. } => {
                    if local == &"default".to_string() {
                        added |= exported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::Default);
                    } else {
                        added |= exported_tree_shake_module
                            .used_exports
                            .add_used_export(&module::UsedIdent::SwcIdent(strip_context(local)));
                    }
                }
                statement_graph::ExportSpecifierInfo::Default => {
                    added |= exported_tree_shake_module
                        .used_exports
                        .add_used_export(&module::UsedIdent::Default);
                }
                statement_graph::ExportSpecifierInfo::All(used_idents) => {
                    if has_side_effects {
                        added |= exported_tree_shake_module.used_exports.use_all();
                    } else if let Some(used_idents) = used_idents {
                        for ident in used_idents {
                            if ident == "*" {
                                added |= exported_tree_shake_module.used_exports.use_all();
                            } else if exported_tree_shake_module.all_exports.contains(ident) {
                                added |= exported_tree_shake_module
                                    .used_exports
                                    .add_used_export(&strip_context(ident));
                            }
                        }
                    } else {
                        added |= exported_tree_shake_module.used_exports.use_all();
                    }
                }
            }
        }
        return if added {
            Some(exported_tree_shake_module.topo_order)
        } else {
            None
        };
    }
    None
}

pub fn strip_context(ident: &str) -> String {
    let ident_split = ident.split('#').collect::<Vec<_>>();
    ident_split[0].to_string()
}
