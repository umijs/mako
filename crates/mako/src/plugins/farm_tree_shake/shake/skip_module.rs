use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::util::take::Take;
use swc_core::ecma::ast::{
    ExportSpecifier, Ident, ImportSpecifier, ModuleDecl, ModuleExportName, ModuleItem,
};
use swc_core::ecma::utils::{quote_ident, quote_str};
use swc_core::quote;

use crate::analyze_deps::analyze_deps;
use crate::ast::js_ast_to_code;
use crate::compiler::Context;
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{is_ident_sym_equal, TreeShakeModule};
use crate::plugins::farm_tree_shake::shake::strip_context;
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportSpecifierInfo, ImportSpecifierInfo, StatementId,
};
use crate::resolve::{resolve, ResolverResource};

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
                quote!("export { default as $ident } from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
            ReExportType2::Namespace => {
                quote!("export * as $ident from \"$from\";" as ModuleItem,
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
            ReExportType2::Namespace => {
                quote!("import * as $ident from \"$from\";" as ModuleItem,
                    ident: Ident = ident,
                    from: Str = quote_str!(self.from_module_id.id.clone())
                )
            }
        }
    }
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
            ReExportType2::Namespace => "*".into(),
        }
    }
}

#[derive(Debug)]
pub enum ReExportType2 {
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

pub(super) fn skip_module_optimize(
    module_graph: &mut ModuleGraph,

    tree_shake_modules_ids: &Vec<ModuleId>,
    tree_shake_modules_map: &HashMap<ModuleId, RefCell<TreeShakeModule>>,

    context: &Arc<Context>,
) -> Result<()> {
    let mut re_export_replace_map: HashMap<ModuleId, Vec<(StatementId, Vec<ReExportReplace>)>> =
        HashMap::new();

    let mut current_index: usize = 0;
    let len = tree_shake_modules_ids.len();

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

    while current_index < len {
        let current_module_id = &tree_shake_modules_ids[current_index];

        let mut replaces = vec![];

        if let Some(tsm) = tree_shake_modules_map.get(current_module_id) {
            let tsm = tsm.borrow();

            for stmt in tsm.stmt_graph.stmts() {
                let mut stmt_replaces = vec![];

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
                    && let Some(_source) = &export_info.source
                {
                    if let Some(tsm_ref) = get_imported_tree_shake_module(
                        current_module_id,
                        _source,
                        module_graph,
                        tree_shake_modules_map,
                    ) {
                        let proxy_tsm = tsm_ref.borrow();

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
                    replaces.push((stmt.id, stmt_replaces));
                }
            }
        }

        if !replaces.is_empty() {
            replaces.sort_by(|(stmt_id_1, _), (stmt_id_2, _)| stmt_id_2.cmp(stmt_id_1));

            re_export_replace_map.insert(current_module_id.clone(), replaces);
        }

        current_index += 1;
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
                let (_code, _) = js_ast_to_code(swc_module, context, &module_id.id).unwrap();

                tsm.update_stmt_graph(swc_module);
            }

            let deps = analyze_deps(&module.info.as_mut().unwrap().ast, &module_id.id, context)?;

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

        if let Some(re_export_source) = proxy_tsm.find_export_source(used_ident) {
            if let Some(source) = &re_export_source.source {
                if let Some(next_tsm_rc) =
                    get_imported_tree_shake_module(proxy_module_id, source, module_graph, tsm_map)
                {
                    let next_tsm = next_tsm_rc.borrow();

                    if matches!(re_export_source.re_export_type, ReExportType2::Namespace) {
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
