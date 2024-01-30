mod find_export_source;
mod module_concatenate;
mod skip_module;

use std::cell::RefCell;
use std::ops::DerefMut;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::comments::{Comment, CommentKind};
use mako_core::swc_common::{DUMMY_SP, GLOBALS};

use self::skip_module::skip_module_optimize;
use crate::compiler::Context;
use crate::module::{ModuleAst, ModuleId, ModuleType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{ModuleSystem, TreeShakeModule};
use crate::plugins::farm_tree_shake::shake::module_concatenate::optimize_module_graph;
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo, ImportInfo,
};
use crate::plugins::farm_tree_shake::{module, remove_useless_stmts, statement_graph};

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
/// 4. remove used module and update tree-shaken AST into module graph
pub fn optimize_farm(module_graph: &mut ModuleGraph, context: &Arc<Context>) -> Result<()> {
    let (topo_sorted_modules, _cyclic_modules) = {
        mako_core::mako_profile_scope!("tree shake topo-sort");
        module_graph.toposort()
    };

    #[cfg(debug_assertions)]
    {
        use mako_core::tracing::debug;
        if !_cyclic_modules.is_empty() {
            debug!("{} cycles in project", _cyclic_modules.len());

            for circle in &_cyclic_modules {
                let circle_str = circle
                    .iter()
                    .map(|i| i.id.clone())
                    .collect::<Vec<_>>()
                    .join("\n");

                debug!("{}:\n{}", circle.len(), circle_str);
            }
        }
    }

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

        let tree_shake_module = GLOBALS.set(&context.meta.script.globals, || {
            TreeShakeModule::new(module, order, module_graph)
        });

        if std::env::var("TS_DEBUG").is_ok() {
            let mut comments = context.meta.script.output_comments.write().unwrap();

            for s in tree_shake_module.stmt_graph.stmts() {
                if s.is_self_executed {
                    comments.add_leading_comment_at(
                        s.span.lo,
                        Comment {
                            kind: CommentKind::Line,
                            span: DUMMY_SP,
                            text: "__SELF_EXECUTED__".into(),
                        },
                    );
                }
            }
        }

        order += 1;
        tree_shake_modules_ids.push(tree_shake_module.module_id.clone());
        tree_shake_modules_map.insert(
            tree_shake_module.module_id.clone(),
            RefCell::new(tree_shake_module),
        );
    }

    let mut current_index = (tree_shake_modules_ids.len() - 1) as i64;

    // update tree-shake module side_effects flag in reversed topo-sort order
    while current_index >= 0 {
        let mut next_index = current_index - 1;
        let module_id = &tree_shake_modules_ids[current_index as usize];

        let mut current_tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();
        let side_effects = current_tsm.update_side_effect();
        drop(current_tsm);

        if side_effects {
            module_graph
                .get_dependents(module_id)
                .iter()
                .for_each(|&(module_id, dependency)| {
                    if let Some(tsm) = tree_shake_modules_map.get(module_id) {
                        let mut tsm = tsm.borrow_mut();
                        if tsm
                            .side_effect_dep_sources
                            .insert(dependency.source.clone())
                            && greater_equal_than(tsm.topo_order, next_index)
                        {
                            next_index = tsm.topo_order as i64;
                        }
                    }
                });
        }

        current_index = next_index;
    }

    // fill tree shake module all exported ident in reversed topo-sort order
    for module_id in tree_shake_modules_ids.iter().rev() {
        let mut tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();

        for exp_info in tsm.exports() {
            if let Some(source) = exp_info.source {
                for sp_info in exp_info.specifiers {
                    match sp_info {
                        // export * from "xx"
                        ExportSpecifierInfo::All(_) | ExportSpecifierInfo::Ambiguous(_) => {
                            if let Some(dependent_id) =
                                module_graph.get_dependency_module_by_source(module_id, &source)
                            {
                                if let Some(rc) = tree_shake_modules_map.get(dependent_id) {
                                    let dependence_module = rc.borrow();

                                    let to_extend = dependence_module.all_exports.clone();

                                    tsm.stmt_graph.stmt_mut(&exp_info.stmt_id).export_info =
                                        Some(ExportInfo {
                                            source: Some(source.clone()),
                                            specifiers: vec![to_extend.to_all_specifier()],
                                            stmt_id: exp_info.stmt_id,
                                        });

                                    tsm.extends_exports(&to_extend);
                                }
                            }
                        }
                        _ => {
                            tsm.all_exports.add_idents(sp_info.to_idents());
                        }
                    }
                }
            } else {
                for exp_sp in exp_info.specifiers {
                    let idents = exp_sp.to_idents();
                    tsm.all_exports.add_idents(idents);
                }
            }
        }
    }

    if let Some(optimization) = &context.config.optimization
        && optimization.skip_modules.unwrap_or(false)
    {
        skip_module_optimize(
            module_graph,
            &tree_shake_modules_ids,
            &tree_shake_modules_map,
            context,
        )?;

        for module_id in tree_shake_modules_ids.iter().rev() {
            let mut tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();

            for exp_info in tsm.exports() {
                if let Some(source) = exp_info.source {
                    for sp_info in exp_info.specifiers {
                        match sp_info {
                            // export * from "xx"
                            ExportSpecifierInfo::All(_) | ExportSpecifierInfo::Ambiguous(_) => {
                                if let Some(dependent_id) =
                                    module_graph.get_dependency_module_by_source(module_id, &source)
                                {
                                    if let Some(rc) = tree_shake_modules_map.get(dependent_id) {
                                        let dependence_module = rc.borrow();

                                        let to_extend = dependence_module.all_exports.clone();

                                        tsm.stmt_graph.stmt_mut(&exp_info.stmt_id).export_info =
                                            Some(ExportInfo {
                                                source: Some(source.clone()),
                                                specifiers: vec![to_extend.to_all_specifier()],
                                                stmt_id: exp_info.stmt_id,
                                            });

                                        tsm.extends_exports(&to_extend);
                                    }
                                }
                            }
                            _ => {
                                tsm.all_exports.add_idents(sp_info.to_idents());
                            }
                        }
                    }
                } else {
                    for exp_sp in exp_info.specifiers {
                        let idents = exp_sp.to_idents();
                        tsm.all_exports.add_idents(idents);
                    }
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
            drop(tree_shake_module);
            for (dep_id, _) in module_graph.get_dependencies(tree_shake_module_id) {
                if let Some(ref_cell) = tree_shake_modules_map.get(dep_id) {
                    let mut dep_module = ref_cell.borrow_mut();

                    if dep_module.use_all_exports() && dep_module.topo_order < next_index {
                        next_index = dep_module.topo_order;
                    }
                }
            }
        } else {
            if tree_shake_module.not_used() {
                //if the module's used_exports is empty, means this module is not used and will be removed
                current_index = next_index;

                continue;
            }

            let side_effects = tree_shake_module.side_effects;

            let module = module_graph
                .get_module_mut(&tree_shake_module.module_id)
                .unwrap();
            let ast = &module.info.as_ref().unwrap().ast;

            if let ModuleAst::Script(_) = ast {
                // remove useless statements and useless imports/exports identifiers, then all preserved import info and export info will be added to the used_exports.
                let info = module.info.as_mut().unwrap();

                // empty optimis info
                info.optims.clear();

                let (used_imports, used_exports_from) =
                    remove_useless_stmts::mark_useless_stmts(tree_shake_module.deref_mut(), info);

                // 解决模块自己引用自己，导致 tree_shake_module 同时存在多个可变引用
                drop(tree_shake_module);

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
                        side_effects,
                        &export_info,
                    ) {
                        if next_index > order {
                            next_index = order;
                        }
                    }
                }
            }
        }

        // add all dynamic imported dependencies as [UsedExports::All]
        for (dep, edge) in module_graph.get_dependencies(tree_shake_module_id) {
            match edge.resolve_type {
                ResolveType::DynamicImport | ResolveType::Worker => {
                    if let Some(ref_cell) = tree_shake_modules_map.get(dep) {
                        let mut tree_shake_module = ref_cell.borrow_mut();
                        if tree_shake_module.use_all_exports()
                            && tree_shake_module.topo_order < next_index
                        {
                            next_index = tree_shake_module.topo_order;
                        }

                        tree_shake_module.side_effects = true;
                    }
                }
                ResolveType::Require => {
                    if let Some(ref_cell) = tree_shake_modules_map.get(dep) {
                        let mut tree_shake_module = ref_cell.borrow_mut();

                        if tree_shake_module.use_all_exports()
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

    for (module_id, tsm) in &tree_shake_modules_map {
        let tsm = tsm.borrow();

        if tsm.not_used() {
            module_graph.module_to_useless(module_id);
        }
    }

    if context
        .config
        .optimization
        .as_ref()
        .map_or(false, |o| o.concatenate_modules.unwrap_or(false))
    {
        optimize_module_graph(module_graph, &tree_shake_modules_map, context)?;
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
    if let Some(imported_module_id) =
        module_graph.get_dependency_module_by_source(tree_shake_module_id, &import_info.source)
    {
        let imported_module = module_graph.get_module(imported_module_id).unwrap();

        if imported_module.get_module_type() == ModuleType::PlaceHolder {
            return None;
        }

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

        //  import "xxx"
        if import_info.specifiers.is_empty() {
            if imported_tree_shake_module.add_used_export(None) {
                return Some(imported_tree_shake_module.topo_order);
            }
            return None;
        }

        let mut added = false;

        for sp in &import_info.specifiers {
            match sp {
                statement_graph::ImportSpecifierInfo::Namespace(_) => {
                    added |= imported_tree_shake_module.use_all_exports();
                }
                statement_graph::ImportSpecifierInfo::Named { local, imported } => {
                    if let Some(ident) = imported {
                        if *ident == "default" {
                            added |= imported_tree_shake_module
                                .add_used_export(Some(&module::UsedIdent::Default));
                        } else {
                            added |= imported_tree_shake_module.add_used_export(Some(
                                &module::UsedIdent::SwcIdent(strip_context(ident)),
                            ));
                        }
                    } else {
                        added |= imported_tree_shake_module.add_used_export(Some(
                            &module::UsedIdent::SwcIdent(strip_context(local)),
                        ));
                    }
                }
                statement_graph::ImportSpecifierInfo::Default(_) => {
                    added |= imported_tree_shake_module
                        .add_used_export(Some(&module::UsedIdent::Default));
                }
            }
        }

        if added {
            Some(imported_tree_shake_module.topo_order)
        } else {
            None
        }
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
        if let Some(exported_module_id) =
            module_graph.get_dependency_module_by_source(tree_shake_module_id, source)
        {
            let exported_module = module_graph.get_module(exported_module_id).unwrap();

            if exported_module.is_external() || exported_module.is_placeholder() {
                return None;
            };

            let mut exported_tree_shake_module = tree_shake_modules_map
                .get(exported_module_id)
                .unwrap()
                .borrow_mut();

            let mut added = false;

            if !export_info.specifiers.is_empty() {
                for sp in &export_info.specifiers {
                    match sp {
                        statement_graph::ExportSpecifierInfo::Namespace(_) => {
                            added |= exported_tree_shake_module.use_all_exports();
                        }
                        statement_graph::ExportSpecifierInfo::Named { local, .. } => {
                            if local == &"default".to_string() {
                                added |= exported_tree_shake_module
                                    .add_used_export(Some(&module::UsedIdent::Default));
                            } else {
                                added |= exported_tree_shake_module.add_used_export(Some(
                                    &module::UsedIdent::SwcIdent(strip_context(local)),
                                ));
                            }
                        }
                        statement_graph::ExportSpecifierInfo::Default(_) => {
                            added |= exported_tree_shake_module
                                .add_used_export(Some(&module::UsedIdent::Default));
                        }
                        statement_graph::ExportSpecifierInfo::All(used_idents) => {
                            if false {
                                added |= exported_tree_shake_module.use_all_exports();
                            } else if used_idents.is_empty() {
                                added |= exported_tree_shake_module.add_used_export(None);
                            } else {
                                for ident in used_idents {
                                    if ident == "*" {
                                        added |= exported_tree_shake_module.use_all_exports();
                                    } else {
                                        added |= exported_tree_shake_module
                                            .add_used_export(Some(&strip_context(ident)));
                                    }
                                }
                            }
                        }
                        statement_graph::ExportSpecifierInfo::Ambiguous(used_idents) => {
                            if has_side_effects {
                                added |= exported_tree_shake_module.use_all_exports();
                            } else {
                                for ident in used_idents {
                                    if ident == "*" {
                                        added |= exported_tree_shake_module.use_all_exports();
                                    } else {
                                        added |= exported_tree_shake_module
                                            .add_used_export(Some(&strip_context(ident)));
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                added |= exported_tree_shake_module.add_used_export(None);
            }
            return if added {
                Some(exported_tree_shake_module.topo_order)
            } else {
                None
            };
        }
    }
    None
}

pub fn strip_context(ident: &str) -> String {
    let ident_split = ident.split('#').collect::<Vec<_>>();
    ident_split[0].to_string()
}
// is a greater than b
fn greater_equal_than(a: usize, b: i64) -> bool {
    if b < 0 {
        true
    } else {
        (a as i64) >= b
    }
}
