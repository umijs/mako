use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use swc_core::common::util::take::Take;
use swc_core::common::{Span, Spanned};
use swc_core::ecma::ast::{
    ExportSpecifier, Ident, ImportSpecifier, ModuleDecl, ModuleExportName, ModuleItem,
};
use swc_core::ecma::utils::{quote_ident, quote_str};
use swc_core::quote;

use crate::compiler::Context;
use crate::module::{Dependency, ImportType, ModuleId, NamedExportType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::tree_shaking::module::{is_ident_sym_equal, TreeShakeModule};
use crate::plugins::tree_shaking::shake::strip_context;
use crate::plugins::tree_shaking::statement_graph::{
    ExportSpecifierInfo, ImportSpecifierInfo, StatementId,
};
use crate::{mako_profile_function, mako_profile_scope, DUMMY_CTXT};

#[derive(Debug)]
pub struct ReExportReplace {
    pub(crate) re_export_ident: String,
    pub(crate) re_export_source: ReExportSource,
    pub(crate) from_module_id: ModuleId,
}

impl ReExportReplace {
    pub(crate) fn to_export_module_item(&self, ident: Ident) -> ModuleItem {
        match &self.re_export_source.re_export_type {
            ReExportType::Default => {
                quote!("export { default as $ident } from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType::Namespace => {
                quote!("export * as $ident from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType::Named(local) => {
                if ident.sym.eq(local) {
                    quote!("export { $ident } from \"$from\";" as ModuleItem,
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                } else {
                    quote!("export { $local as $ident } from \"$from\";" as ModuleItem,
                        local: Ident = quote_ident!(DUMMY_CTXT, local.clone()),
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                }
            }
        }
    }

    pub(crate) fn to_import_dep(&self, span: Span) -> Dependency {
        let import_type: ImportType = (&self.re_export_source.re_export_type).into();

        Dependency {
            source: self.from_module_id.id.clone(),
            span: Some(span),
            order: 0,
            resolve_as: None,
            resolve_type: ResolveType::Import(import_type),
        }
    }

    pub(crate) fn to_export_dep(&self, span: Span) -> Dependency {
        let resolve_type = match &self.re_export_source.re_export_type {
            ReExportType::Namespace => ResolveType::ExportNamed(NamedExportType::Namespace),
            ReExportType::Default => ResolveType::ExportNamed(NamedExportType::Default),
            ReExportType::Named(_) => ResolveType::ExportNamed(NamedExportType::Named),
        };

        Dependency {
            source: self.from_module_id.id.clone(),
            resolve_as: None,
            resolve_type,
            order: 0,
            span: Some(span),
        }
    }

    pub(crate) fn to_import_module_item(&self, ident: Ident) -> ModuleItem {
        match &self.re_export_source.re_export_type {
            ReExportType::Default => {
                quote!("import $ident from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType::Named(local) => {
                if ident.sym.eq(local) {
                    quote!("import { $ident } from \"$from\";" as ModuleItem,
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                } else {
                    quote!("import { $local as $ident } from \"$from\";" as ModuleItem,
                        local: Ident = quote_ident!(DUMMY_CTXT,local.clone()),
                        ident: Ident = ident,
                        from: Str = quote_str!(self.from_module_id.id.clone())
                    )
                }
            }
            ReExportType::Namespace => {
                quote!("import * as $ident from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
        }
    }
}

#[derive(Debug)]
pub struct ReExportSource {
    pub(crate) source: Option<String>,
    pub(crate) re_export_type: ReExportType,
}

impl ReExportSource {
    pub fn to_outer_ref(&self) -> String {
        match &self.re_export_type {
            ReExportType::Default => "default".into(),
            ReExportType::Named(local) => local.clone(),
            ReExportType::Namespace => "*".into(),
        }
    }
}

#[derive(Debug)]
pub enum ReExportType {
    // export * as x from "x"
    Namespace,
    // import x from "y"
    // export x from "y"
    Default,

    // import {x as z} from "y"
    // export {x as z} from "y"
    // export     *    from "y"
    // export {x as z}
    Named(String),
}

impl From<&ReExportType> for ImportType {
    fn from(re_export_type: &ReExportType) -> Self {
        match re_export_type {
            ReExportType::Namespace => ImportType::Namespace,
            ReExportType::Default => ImportType::Default,
            ReExportType::Named(_) => ImportType::Named,
        }
    }
}

pub(super) fn skip_module_optimize(
    module_graph: &mut ModuleGraph,
    tree_shake_modules_ids: &[ModuleId],
    tree_shake_modules_map: &HashMap<ModuleId, RefCell<TreeShakeModule>>,
    _context: &Arc<Context>,
) -> Result<()> {
    mako_profile_function!();

    let mut re_export_replace_map: HashMap<
        ModuleId,
        Vec<(StatementId, Vec<ReExportReplace>, String)>,
    > = HashMap::new();

    let mut current_index: usize = 0;
    let len = tree_shake_modules_ids.len();

    fn apply_replace(
        to_replace: &(StatementId, Vec<ReExportReplace>, String),
        module_id: &ModuleId,
        module_graph: &mut ModuleGraph,
    ) {
        let stmt_id = to_replace.0;
        let replaces = &to_replace.1;
        let source = &to_replace.2;

        let module = module_graph.get_module_mut(module_id).unwrap();

        let swc_module = module.info.as_mut().unwrap().ast.as_script_ast_mut();

        let mut stmt = swc_module.body.get(stmt_id).unwrap().clone();
        let mut to_insert = vec![];
        let mut to_insert_deps = vec![];
        let mut to_delete = false;
        let mut resolve_type: Option<ResolveType> = None;

        match &mut stmt {
            ModuleItem::ModuleDecl(module_decl) => match module_decl {
                ModuleDecl::Import(import_decl) => {
                    resolve_type = Some(ResolveType::Import(ImportType::empty()));

                    for replace in replaces {
                        let mut matched_index = None;
                        let mut matched_ident = None;

                        for (index, specifier) in import_decl.specifiers.iter_mut().enumerate() {
                            match specifier {
                                ImportSpecifier::Named(named) => {
                                    let match_ident = named.imported.as_ref().map_or_else(
                                        || named.local.clone().sym.to_string(),
                                        |i| match i {
                                            ModuleExportName::Ident(ident) => {
                                                ident.sym.clone().to_string()
                                            }
                                            ModuleExportName::Str(_) => {
                                                unreachable!();
                                            }
                                        },
                                    );

                                    if match_ident == replace.re_export_ident {
                                        matched_ident = Some(named.local.take());
                                        matched_index = Some(index);
                                        break;
                                    }
                                }
                                ImportSpecifier::Default(_default_specifier) => {}
                                ImportSpecifier::Namespace(_) => {
                                    // import * as not allowed
                                    continue;
                                }
                            }
                        }
                        let removed_specifier =
                            matched_index.map(|i| import_decl.specifiers.remove(i));

                        to_delete = import_decl.specifiers.is_empty();

                        if let Some(ident) = matched_ident
                            && removed_specifier.is_some()
                        {
                            let module_item = replace.to_import_module_item(ident);
                            let dep = replace.to_import_dep(import_decl.span());

                            to_insert.push(module_item);
                            to_insert_deps.push(dep);
                        }
                    }
                }
                ModuleDecl::ExportDecl(_) => {}
                ModuleDecl::ExportNamed(export_named) => {
                    if export_named.src.is_some() {
                        resolve_type = Some(ResolveType::ExportNamed(NamedExportType::empty()));

                        for replace in replaces {
                            let mut matched_index = None;
                            let mut matched_ident = None;

                            for (index, specifier) in export_named.specifiers.iter_mut().enumerate()
                            {
                                match specifier {
                                    ExportSpecifier::Namespace(_) => {}
                                    ExportSpecifier::Default(_) => {
                                        unreachable!("exportDefaultFrom is not supported in mako");
                                    }
                                    ExportSpecifier::Named(named_export_specifier) => {
                                        match &mut named_export_specifier.orig {
                                            ModuleExportName::Ident(ident) => {
                                                if is_ident_sym_equal(
                                                    ident.as_ref(),
                                                    &replace.re_export_ident,
                                                ) {
                                                    let exporeted_ident = named_export_specifier
                                                        .exported
                                                        .clone()
                                                        .unwrap_or_else(|| {
                                                            named_export_specifier.orig.clone()
                                                        });

                                                    match exporeted_ident {
                                                        ModuleExportName::Ident(exported_ident) => {
                                                            matched_ident = Some(exported_ident);
                                                            matched_index = Some(index);
                                                        }
                                                        ModuleExportName::Str(_) => {}
                                                    }
                                                }
                                            }
                                            ModuleExportName::Str(_) => {}
                                        }
                                    }
                                }
                            }

                            let removed_specifier =
                                matched_index.map(|index| export_named.specifiers.remove(index));

                            to_delete = export_named.specifiers.is_empty();

                            if let Some(ident) = matched_ident
                                && removed_specifier.is_some()
                            {
                                let module_item = replace.to_export_module_item(ident);
                                to_insert.push(module_item);
                                to_insert_deps.push(replace.to_export_dep(export_named.span()));
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
            swc_module.body.remove(stmt_id);
            swc_module.body.splice(stmt_id..stmt_id, to_insert);
        } else {
            swc_module.body.splice(stmt_id..stmt_id, to_insert);
        }

        if to_delete {
            module_graph.remove_dependency_module_by_source_and_resolve_type(
                module_id,
                source,
                resolve_type.unwrap(),
            );
        }
        for dep in to_insert_deps {
            module_graph.add_dependency(module_id, &dep.source.clone().into(), dep);
        }
    }

    while current_index < len {
        mako_profile_scope!("skip", &tree_shake_modules_ids[current_index].id);

        let current_module_id = &tree_shake_modules_ids[current_index];

        let mut replaces = vec![];

        if let Some(tsm) = tree_shake_modules_map.get(current_module_id) {
            let tsm = tsm.borrow();

            for stmt in tsm.stmt_graph.stmts() {
                let mut stmt_replaces = vec![];
                let mut stmt_source = None;

                if let Some(import) = &stmt.import_info {
                    if import.specifiers.is_empty() {
                        continue;
                    }

                    // import * as x from "x"
                    if import.specifiers.len() == 1
                        && matches!(&import.specifiers[0], ImportSpecifierInfo::Namespace(_))
                    {
                        continue;
                    }

                    if let Some(imported_tsm_ref) = get_imported_tree_shake_module(
                        current_module_id,
                        &import.source,
                        module_graph,
                        tree_shake_modules_map,
                    ) {
                        let imported_tsm = imported_tsm_ref.borrow();

                        let imported_module_has_side_effects = imported_tsm.has_side_effect();

                        if imported_module_has_side_effects {
                            continue;
                        }

                        stmt_source = Some(import.source.clone());

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

                                    if let Some(re_export_replace) = find_ident_export_source(
                                        module_graph,
                                        tree_shake_modules_map,
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
                                    if let Some(re_export_replace) = find_ident_export_source(
                                        module_graph,
                                        tree_shake_modules_map,
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
                    && let Some(source) = &export_info.source
                {
                    if let Some(tsm_ref) = get_imported_tree_shake_module(
                        current_module_id,
                        source,
                        module_graph,
                        tree_shake_modules_map,
                    ) {
                        let proxy_tsm = tsm_ref.borrow();

                        stmt_source = Some(source.clone());
                        for export_specifier in export_info.specifiers.iter() {
                            match export_specifier {
                                ExportSpecifierInfo::All(_) => {}
                                ExportSpecifierInfo::Named { local, exported: _ } => {
                                    let ref_ident = strip_context(local);

                                    if let Some(re_export_replace) = find_ident_export_source(
                                        module_graph,
                                        tree_shake_modules_map,
                                        &proxy_tsm.module_id,
                                        &ref_ident,
                                    ) && !re_export_replace /* at least one level deeper */
                                        .from_module_id
                                        .eq(&proxy_tsm.module_id)
                                    {
                                        stmt_replaces.push(re_export_replace);
                                    }
                                }
                                ExportSpecifierInfo::Default(_name) => {
                                    if let Some(re_export_replace) = find_ident_export_source(
                                        module_graph,
                                        tree_shake_modules_map,
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
                    replaces.push((stmt.id, stmt_replaces, stmt_source.unwrap()));
                }
            }
        }

        if !replaces.is_empty() {
            replaces.sort_by(|(stmt_id_1, _, _), (stmt_id_2, _, _)| stmt_id_2.cmp(stmt_id_1));

            re_export_replace_map.insert(current_module_id.clone(), replaces);
        }

        current_index += 1;
    }

    for (module_id, replaces) in re_export_replace_map.iter() {
        if module_graph.has_module(module_id) {
            // stmt_id is reversed order
            for to_replace in replaces.iter() {
                // println!("{} apply with {:?}", module_id.id, to_replace.1);
                apply_replace(to_replace, module_id, module_graph);
            }

            let mut tsm = tree_shake_modules_map.get(module_id).unwrap().borrow_mut();

            let swc_module = module_graph
                .get_module(module_id)
                .unwrap()
                .info
                .as_ref()
                .unwrap()
                .ast
                .as_script_ast();

            tsm.update_stmt_graph(swc_module);
        }
    }

    Ok(())
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

fn find_ident_export_source(
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

        if let Some(re_export_source) = proxy_tsm.find_skipable_export_source(used_ident) {
            if let Some(source) = &re_export_source.source {
                if let Some(next_tsm_rc) =
                    get_imported_tree_shake_module(proxy_module_id, source, module_graph, tsm_map)
                {
                    let next_tsm = next_tsm_rc.borrow();

                    if matches!(re_export_source.re_export_type, ReExportType::Namespace) {
                        return Some(ReExportReplace {
                            re_export_ident: used_ident.clone(),
                            from_module_id: next_tsm.module_id.clone(),
                            re_export_source,
                        });
                    }

                    if !next_tsm.has_side_effect() {
                        let ref_ident = re_export_source.to_outer_ref();

                        if let Some(next_replace) = find_ident_export_source(
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
                        } else {
                            return Some(ReExportReplace {
                                re_export_ident: used_ident.clone(),
                                from_module_id: next_tsm.module_id.clone(),
                                re_export_source,
                            });
                        }
                    } else {
                        return Some(ReExportReplace {
                            re_export_ident: used_ident.clone(),
                            from_module_id: next_tsm.module_id.clone(),
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
