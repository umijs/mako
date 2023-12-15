use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::comments::{Comment, CommentKind};
use mako_core::swc_common::util::take::Take;
use mako_core::swc_common::{DUMMY_SP, GLOBALS};
use mako_core::swc_ecma_ast::ModuleItem;
use mako_core::swc_ecma_utils::{quote_ident, quote_str};
use swc_core::ecma::ast::{ExportSpecifier, Ident, ImportSpecifier, ModuleDecl, ModuleExportName};
use swc_core::quote;

use crate::analyze_deps::analyze_deps;
use crate::compiler::Context;
use crate::module::{ModuleAst, ModuleId, ModuleType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{is_ident_sym_equal, TreeShakeModule};
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo, ImportInfo, ImportSpecifierInfo, StatementId,
};
use crate::plugins::farm_tree_shake::{module, remove_useless_stmts, statement_graph};
use crate::resolve::{resolve, ResolverResource};
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
/// 4. remove used module and update tree-shaken AST into module graph
pub fn optimize_farm(module_graph: &mut ModuleGraph, context: &Arc<Context>) -> Result<()> {
    let (topo_sorted_modules, _cyclic_modules) = {
        mako_core::mako_profile_scope!("tree shake topo-sort");
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

    fn get_imported_tree_shake_module<'a>(
        from_module_id: &ModuleId,
        source: &String,
        module_graph: &ModuleGraph,
        tree_shake_modules_map: &'a HashMap<ModuleId, RefCell<TreeShakeModule>>,
    ) -> Option<&'a RefCell<TreeShakeModule>> {
        if let Some(imported_module_id) =
            module_graph.get_dependency_module_by_source(from_module_id, source)
        {
            tree_shake_modules_map.get(imported_module_id)
        } else {
            None
        }
    }

    fn find_reexport_ident_source(
        module_graph: &ModuleGraph,
        tsm_map: &HashMap<ModuleId, RefCell<TreeShakeModule>>,
        proxy_module_id: &ModuleId,
        used_ident: &String,
    ) -> Option<ReExportReplace> {
        if let Some(tsm) = tsm_map.get(proxy_module_id) {
            let proxy_tsm = tsm.borrow();

            if proxy_tsm.has_side_effect() {
                return None;
            }

            if let Some(re_export_source) = proxy_tsm.find_export_define_wip(used_ident) {
                if let Some(source) = &re_export_source.source {
                    if let Some(next_tsm_rc) = get_imported_tree_shake_module(
                        proxy_module_id,
                        source,
                        module_graph,
                        tsm_map,
                    ) {
                        let next_tsm = next_tsm_rc.borrow();

                        if !next_tsm.has_side_effect() {
                            let ref_ident = re_export_source.to_outer_ref();

                            if let Some(next_replace) = find_reexport_ident_source(
                                module_graph,
                                tsm_map,
                                &next_tsm.module_id,
                                &ref_ident,
                            ) {
                                return Some(ReExportReplace {
                                    re_export_ident: used_ident.clone(),
                                    from_module_id: next_replace.from_module_id.clone(),
                                    re_export_source: next_replace.re_export_source,
                                });
                            }
                        } else {
                            return Some(ReExportReplace {
                                re_export_ident: used_ident.clone(),
                                from_module_id: proxy_module_id.clone(),
                                re_export_source,
                            });
                        }
                    }
                } else {
                    return Some(ReExportReplace {
                        re_export_ident: used_ident.clone(),
                        from_module_id: proxy_module_id.clone(),
                        re_export_source,
                    });
                }
            } else {
                return None;
            }
        }

        None
    }

    let mut re_export_replace_map: HashMap<ModuleId, Vec<(StatementId, Vec<ReExportReplace>)>> =
        HashMap::new();

    let mut current_index: usize = 0;
    let len = tree_shake_modules_ids.len();
    while current_index < len {
        let current_module_id = &tree_shake_modules_ids[current_index];

        let mut repalces = vec![];

        if let Some(tsm) = tree_shake_modules_map.get(current_module_id) {
            let tsm = tsm.borrow();
            for stmt in tsm.stmt_graph.stmts() {
                let mut stmt_replaces = vec![];

                if let Some(import) = &stmt.import_info {
                    //TODO

                    if import.specifiers.is_empty() {
                        continue;
                    }

                    if import.specifiers.len() == 1
                        && matches!(&import.specifiers[0], ImportSpecifierInfo::Namespace(_))
                    {
                        continue;
                    }

                    if let Some(imported_tsm_ref) = get_imported_tree_shake_module(
                        current_module_id,
                        &import.source,
                        module_graph,
                        &tree_shake_modules_map,
                    ) {
                        let imported_tsm = imported_tsm_ref.borrow();

                        let imported_module_has_side_effects = imported_tsm.has_side_effect();

                        if imported_module_has_side_effects {
                            continue;
                        }

                        for sp in &import.specifiers {
                            match sp {
                                ImportSpecifierInfo::Namespace(_) => {
                                    // cant optimize namespace import
                                }
                                ImportSpecifierInfo::Named { local, imported } => {
                                    let imported_ident = strip_context(
                                        &imported
                                            .as_ref()
                                            .map_or_else(|| local.clone(), |i| i.clone()),
                                    );

                                    if let Some(re_export_replace) = find_reexport_ident_source(
                                        module_graph,
                                        &tree_shake_modules_map,
                                        &imported_tsm.module_id,
                                        &imported_ident,
                                    ) && !re_export_replace /* at least one level deeper */
                                        .from_module_id
                                        .eq(&imported_tsm.module_id)
                                    {
                                        stmt_replaces.push(re_export_replace);
                                    }
                                }
                                ImportSpecifierInfo::Default(_) => {
                                    if let Some(re_export_replace) = find_reexport_ident_source(
                                        module_graph,
                                        &tree_shake_modules_map,
                                        &imported_tsm.module_id,
                                        &"default".to_string(),
                                    ) && !re_export_replace /* at least one level deeper */
                                        .from_module_id
                                        .eq(&imported_tsm.module_id)
                                    {
                                        stmt_replaces.push(re_export_replace);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(export_info) = &stmt.export_info
                    && let Some(_source) = &export_info.source
                {
                    if let Some(tsm_ref) = get_imported_tree_shake_module(
                        current_module_id,
                        _source,
                        module_graph,
                        &tree_shake_modules_map,
                    ) {
                        let proxy_tsm = tsm_ref.borrow();

                        for export_specifier in export_info.specifiers.iter() {
                            match export_specifier {
                                ExportSpecifierInfo::All(_) => {}
                                ExportSpecifierInfo::Named { local, exported: _ } => {
                                    let ref_ident = strip_context(local);

                                    if let Some(re_export_replace) = find_reexport_ident_source(
                                        module_graph,
                                        &tree_shake_modules_map,
                                        &proxy_tsm.module_id,
                                        &ref_ident,
                                    ) && !re_export_replace /* at least one level deeper */
                                        .from_module_id
                                        .eq(&proxy_tsm.module_id)
                                    {
                                        stmt_replaces.push(re_export_replace);
                                    }
                                }
                                ExportSpecifierInfo::Default(_) => {
                                    if let Some(re_export_replace) = find_reexport_ident_source(
                                        module_graph,
                                        &tree_shake_modules_map,
                                        &proxy_tsm.module_id,
                                        &"default".to_string(),
                                    ) && !re_export_replace /* at least one level deeper */
                                        .from_module_id
                                        .eq(&proxy_tsm.module_id)
                                    {
                                        stmt_replaces.push(re_export_replace);
                                    }
                                }
                                ExportSpecifierInfo::Namespace(_) => {}
                                ExportSpecifierInfo::Ambiguous(_) => {}
                            }
                        }
                    }
                }

                if !stmt_replaces.is_empty() {
                    repalces.push((stmt.id, stmt_replaces));
                }
            }
        }

        if !repalces.is_empty() {
            repalces.sort_by(|(stmt_id_1, _), (stmt_id_2, _)| stmt_id_2.cmp(stmt_id_1));

            re_export_replace_map.insert(current_module_id.clone(), repalces);
        }

        current_index += 1;
    }

    fn apply_replace(body: &mut Vec<ModuleItem>, to_replace: &(StatementId, Vec<ReExportReplace>)) {
        let stmt_id = to_replace.0;
        let replaces = &to_replace.1;

        let mut stmt = body.get(stmt_id).unwrap().clone();
        let mut to_insert = vec![];
        let mut to_delete = false;

        match &mut stmt {
            ModuleItem::ModuleDecl(module_decl) => match module_decl {
                ModuleDecl::Import(import_decl) => {
                    for replace in replaces {
                        let mut matched_index = None;
                        let mut matched_ident = None;

                        for (index, specifier) in import_decl.specifiers.iter_mut().enumerate() {
                            match specifier {
                                ImportSpecifier::Named(named) => {
                                    if named.local.sym == replace.re_export_ident {
                                        matched_ident = Some(named.local.take());
                                        matched_index = Some(index);
                                    }
                                }
                                ImportSpecifier::Default(_default_specifier) => {}
                                ImportSpecifier::Namespace(_) => {
                                    // import * as not allowed
                                    continue;
                                }
                            }
                        }
                        matched_index.map(|i| import_decl.specifiers.remove(i));

                        to_delete = import_decl.specifiers.is_empty();

                        if let Some(module_item) =
                            matched_ident.map(|ident| replace.to_import_module_item(ident))
                        {
                            to_insert.push(module_item);
                        }
                    }
                }
                ModuleDecl::ExportDecl(_) => {}
                ModuleDecl::ExportNamed(export_named) => {
                    if export_named.src.is_some() {
                        for replace in replaces {
                            let mut matched_index = None;
                            let mut matched_ident = None;

                            for (index, specifier) in export_named.specifiers.iter_mut().enumerate()
                            {
                                match specifier {
                                    ExportSpecifier::Namespace(_) => {}
                                    ExportSpecifier::Default(default_specifier) => {
                                        if replace.re_export_ident == "default" {
                                            matched_ident = Some(default_specifier.exported.take());
                                            matched_index = Some(index);
                                        }
                                    }
                                    ExportSpecifier::Named(named_export_specifier) => {
                                        match &mut named_export_specifier.orig {
                                            ModuleExportName::Ident(ident) => {
                                                if is_ident_sym_equal(
                                                    ident.as_ref(),
                                                    &replace.re_export_ident,
                                                ) {
                                                    matched_ident = Some(ident.take());
                                                    matched_index = Some(index);
                                                }
                                            }
                                            ModuleExportName::Str(_) => {}
                                        }
                                    }
                                }
                            }

                            if let Some(index) = matched_index {
                                export_named.specifiers.remove(index);
                            }
                            to_delete = export_named.specifiers.is_empty();

                            if let Some(module_item) =
                                matched_ident.map(|ident| replace.to_export_module_item(ident))
                            {
                                to_insert.push(module_item);
                            }
                        }
                    }
                }
                ModuleDecl::ExportDefaultDecl(_) => {}
                ModuleDecl::ExportDefaultExpr(_) => {}
                ModuleDecl::ExportAll(_) => {
                    // TODO
                }
                ModuleDecl::TsImportEquals(_)
                | ModuleDecl::TsExportAssignment(_)
                | ModuleDecl::TsNamespaceExport(_) => {
                    unreachable!("TS Type never goes here")
                }
            },
            ModuleItem::Stmt(_) => {}
        }

        if to_delete {
            body.remove(stmt_id);
            body.splice(stmt_id..stmt_id, to_insert);
        } else {
            body.splice(stmt_id..stmt_id, to_insert);
        }
    }

    for (module_id, replaces) in re_export_replace_map.iter() {
        if let Some(module) = module_graph.get_module_mut(module_id) {
            {
                let swc_module = module.info.as_mut().unwrap().ast.as_script_mut();

                // stmt_id is reversed order
                for to_replace in replaces.iter() {
                    // println!("{} apply with {:?}", module_id.id, to_replace.1);
                    apply_replace(&mut swc_module.body, to_replace)
                }

                let mut tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();

                tsm.update_stmt_graph(swc_module);
            }

            let deps = analyze_deps(&module.info.as_mut().unwrap().ast, context)?;

            module_graph.remove_dependencies(module_id);

            for dep in deps.iter() {
                let ret = resolve(&module_id.id, dep, &context.resolvers, context)?;

                match &ret {
                    ResolverResource::Resolved(_) => {
                        let resolved_module_id: ModuleId = ret.get_resolved_path().into();

                        module_graph.add_dependency(module_id, &resolved_module_id, dep.clone());
                    }
                    ResolverResource::External(_) | ResolverResource::Ignored => {}
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
            let ast = &mut module.info.as_mut().unwrap().ast;

            if let ModuleAst::Script(swc_module) = ast {
                // remove useless statements and useless imports/exports identifiers, then all preserved import info and export info will be added to the used_exports.

                let mut shadow = swc_module.ast.clone();

                let (used_imports, used_exports_from) = remove_useless_stmts::remove_useless_stmts(
                    tree_shake_module.deref_mut(),
                    &mut shadow,
                );

                tree_shake_module.updated_ast = Some(shadow);

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

    for (module_id, tsm) in tree_shake_modules_map {
        let tsm = tsm.borrow();

        if tsm.not_used() {
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
    let module_id = module_graph
        .get_dependency_module_by_source(tree_shake_module_id, &import_info.source)
        .map_or_else(
            || {
                let module_id: ModuleId = import_info.source.clone().into();
                Some(module_id)
            },
            |i| Some(i.clone()),
        );

    if let Some(imported_module_id) = &module_id {
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
                    imported_tree_shake_module.use_all_exports();
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

            if exported_module.is_external() {
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

#[derive(Debug)]
pub struct ReExportReplace {
    pub(crate) re_export_ident: String,
    pub(crate) re_export_source: ReExportSource2,
    pub(crate) from_module_id: ModuleId,
}

impl ReExportReplace {
    pub(crate) fn to_export_module_item(&self, ident: Ident) -> ModuleItem {
        match &self.re_export_source.re_export_type {
            ReExportType2::Default => {
                quote!("export { default as $ident } \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType2::Named(local) => {
                if ident.sym.eq(local) {
                    quote!("export { $ident } from \"$from\";" as ModuleItem,
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                } else {
                    quote!("export { $local as $ident } from \"$from\";" as ModuleItem,
                        local: Ident = quote_ident!(local.clone()),
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                }
            }
        }
    }

    pub(crate) fn to_import_module_item(&self, ident: Ident) -> ModuleItem {
        match &self.re_export_source.re_export_type {
            ReExportType2::Default => {
                quote!("import $ident from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType2::Named(local) => {
                if ident.sym.eq(local) {
                    quote!("import { $ident } from \"$from\";" as ModuleItem,
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                } else {
                    quote!("import { $local as $ident } from \"$from\";" as ModuleItem,
                        local: Ident = quote_ident!(local.clone()),
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                }
            }
        }
    }

    pub(crate) fn to_export_stmt(&self) {
        todo!()
    }

    // pub fn to_import_module_decl(&self, local_ident: &String) -> ModuleItem {
    //     let local_ident = quote_ident!(local_ident.clone());
    //
    //     match &self.re_export_type {
    //         ReExportType::Namespace(_) => {
    //             quote!("import * as $ident from \"$from\";" as ModuleItem,
    //                 ident: Ident = local_ident,
    //                 from: Str = quote_str!(self.module_id.id.clone()))
    //         }
    //         ReExportType::Default => quote!("import * as $ident from \"$from\";" as ModuleItem,
    //                 ident: Ident = local_ident,
    //                 from: Str = quote_str!(self.module_id.id.clone())),
    //         ReExportType::Named(local, imporeted) => {
    //             quote!("import * as $ident from \"$from\";" as ModuleItem,
    //                 ident: Ident = local_ident,
    //                 from: Str = quote_str!(self.module_id.id.clone()))
    //         }
    //     }
    // }
}

#[derive(Debug)]
pub struct ReExportSource {
    pub(crate) source: Option<String>,
    pub(crate) re_export_type: ReExportType,
}

#[derive(Debug)]
pub struct ReExportSource2 {
    pub(crate) source: Option<String>,
    pub(crate) re_export_type: ReExportType2,
}

impl ReExportSource2 {
    pub fn to_outer_ref(&self) -> String {
        match &self.re_export_type {
            ReExportType2::Default => "default".into(),
            ReExportType2::Named(local) => local.clone(),
        }
    }
}

impl ReExportSource {
    pub fn to_outer_ref(&self) -> String {
        match &self.re_export_type {
            ReExportType::Namespace(_) => {
                todo!();
            }
            ReExportType::Default => {
                todo!();
            }
            ReExportType::Named(local, imported) => {
                if let Some(imported) = imported {
                    imported.clone()
                } else {
                    local.clone()
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ReExportType2 {
    // export * as x from "y"
    // Namespace export not supported
    // Namespace(String),

    // import x from "y"
    // export x from "y"
    Default,

    // import {x as z} from "y"
    // export {x as z} from "y"
    // export     *    from "y"
    // export {x as z}
    Named(String),
}
#[derive(Debug)]
pub enum ReExportType {
    // export * as x from "y"
    Namespace(String),
    // import x from "y"
    // export x from "y"
    Default,

    // import {x as z} from "y"
    // export {x as z} from "y"
    // export     *    from "y"
    // export {x as z}
    Named(String, Option<String>),
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
