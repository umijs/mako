use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::swc_common::util::take::Take;
use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ClassDecl, DefaultDecl, ExportDecl, ExportDefaultDecl, ExportDefaultExpr, ExportNamedSpecifier,
    ExportSpecifier, FnDecl, Id, Ident, ImportDecl, ImportSpecifier, Module, ModuleExportName,
    ModuleItem, NamedExport, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{quote_ident, ExprFactory, IdentRenamer};
use swc_core::ecma::visit::{VisitMut, VisitMutWith, VisitWith};

use crate::compiler::Context;
use crate::module::{relative_to_root, ModuleId};
use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::{
    ConcatenateContext, ModuleRef, ModuleRefMap,
};
use crate::plugins::farm_tree_shake::shake::module_concatenate::exports_transform::collect_exports_map;
use crate::plugins::farm_tree_shake::shake::module_concatenate::inner_transformer::InnerOrExternal;
use crate::plugins::farm_tree_shake::shake::module_concatenate::module_ref_rewriter::ModuleRefRewriter;
use crate::plugins::farm_tree_shake::shake::module_concatenate::ref_link::{
    ModuleDeclMapCollector, Symbol, VarLink,
};
use crate::plugins::farm_tree_shake::shake::module_concatenate::utils::{
    declare_var_with_init_stmt, uniq_module_default_export_name,
    MODULE_CONCATENATE_ERROR_STR_MODULE_NAME,
};

pub(super) struct RootTransformer<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub current_module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
    pub import_source_to_module_id: &'a HashMap<String, ModuleId>,
    pub src_to_module: &'a HashMap<String, ModuleId>,

    my_top_decls: HashSet<String>,
    top_level_mark: Mark,

    pub module_id: &'a ModuleId,

    exports: HashMap<String, String>,
    rename_request: Vec<(Id, Id)>,
    current_stmt_index: usize,
    replaces: Vec<(usize, Vec<ModuleItem>)>,
    default_bind_name: String,
}

impl RootTransformer<'_> {
    pub fn new<'a>(
        concatenate_context: &'a mut ConcatenateContext,
        current_module_id: &'a ModuleId,
        context: &'a Arc<Context>,
        import_source_to_module_id: &'a HashMap<String, ModuleId>,
        top_level_mark: Mark,
    ) -> RootTransformer<'a> {
        let default_bind_name = concatenate_context.negotiate_safe_var_name(
            &HashSet::new(),
            &uniq_module_default_export_name(current_module_id),
        );

        RootTransformer {
            concatenate_context,
            current_module_id,
            context,
            module_id: current_module_id,
            exports: Default::default(),
            rename_request: vec![],
            current_stmt_index: 0,
            replaces: vec![],
            import_source_to_module_id,
            src_to_module: import_source_to_module_id,
            top_level_mark,
            default_bind_name,
            my_top_decls: HashSet::new(),
        }
    }

    fn add_comment(&mut self, n: &mut Module) {
        if let Some(first_stmt) = n
            .body
            .iter()
            .find(|&item| matches!(item, ModuleItem::Stmt(_)))
        {
            let mut comments = self.context.meta.script.origin_comments.write().unwrap();

            comments.add_leading_comment_at(
                first_stmt.span_lo(),
                Comment {
                    kind: CommentKind::Line,
                    text: format!(
                        " ROOT MODULE: {}",
                        relative_to_root(&self.current_module_id.id, &self.context.root)
                    )
                    .into(),
                    span: DUMMY_SP,
                },
            );
        }
    }

    fn src_exports(&self, src: &str) -> Option<&HashMap<String, String>> {
        self.import_source_to_module_id
            .get(src)
            .and_then(|module_id| self.concatenate_context.modules_in_scope.get(module_id))
    }
}

impl<'a> RootTransformer<'a> {
    pub(crate) fn to_import_module_ref(&self, var_map: &HashMap<Id, VarLink>) -> ModuleRefMap {
        let mut ref_map = HashMap::new();

        var_map.iter().for_each(|(id, link)| match link {
            VarLink::Direct(direct_id) => {
                ref_map.insert(id.clone(), (direct_id.clone().into(), None));
            }
            VarLink::InDirect(symbol, source) => {
                let src_module_id = self.src_to_module.get(source).unwrap();

                match self.inner_or_external(src_module_id) {
                    InnerOrExternal::Inner(map) => {
                        let module_ref = match symbol {
                            Symbol::Default => map.get("default").unwrap().clone(),
                            Symbol::Namespace => map.get("*").unwrap().clone(),
                            Symbol::Var(ident) => map.get(&ident.sym.to_string()).unwrap().clone(),
                        };

                        ref_map.insert(id.clone(), module_ref);
                    }
                    InnerOrExternal::External(external_names) => {
                        ref_map.insert(
                            id.clone(),
                            (quote_ident!(external_names.1.clone()), symbol.to_field()),
                        );
                    }
                }
            }
        });

        ref_map
    }

    pub(crate) fn to_export_module_ref(
        &self,
        var_map: &HashMap<Id, VarLink>,
    ) -> HashMap<String, ModuleRef> {
        let mut ref_map = HashMap::new();

        var_map.iter().for_each(|(id, link)| match link {
            VarLink::Direct(direct_id) => {
                ref_map.insert(id.0.to_string(), (direct_id.clone().into(), None));
            }
            VarLink::InDirect(symbol, source) => {
                let src_module_id = self.src_to_module.get(source).unwrap();

                match self.inner_or_external(src_module_id) {
                    InnerOrExternal::Inner(map) => {
                        let module_ref = match symbol {
                            Symbol::Default => map.get("default").unwrap().clone(),
                            Symbol::Namespace => map.get("*").unwrap().clone(),
                            Symbol::Var(ident) => map.get(&ident.sym.to_string()).unwrap().clone(),
                        };

                        ref_map.insert(id.0.to_string(), module_ref);
                    }
                    InnerOrExternal::External(external_names) => {
                        ref_map.insert(
                            id.0.to_string(),
                            (quote_ident!(external_names.1.clone()), symbol.to_field()),
                        );
                    }
                }
            }
        });

        ref_map
    }

    fn remove_imported_top_vars(&mut self, import_map: &ModuleRefMap) {
        import_map.iter().for_each(|(id, _)| {
            self.my_top_decls.remove(&id.0.to_string());
        });
    }

    fn inner_or_external(&'a self, module_id: &ModuleId) -> InnerOrExternal<'a> {
        self.concatenate_context
            .external_module_namespace
            .get(module_id)
            .map_or_else(
                || {
                    let map = self
                        .concatenate_context
                        .modules_exports_map
                        .get(module_id)
                        .unwrap();

                    return InnerOrExternal::Inner(map);
                },
                |namespaces| return InnerOrExternal::External(namespaces),
            )
    }

    fn collect_exports(&mut self, n: &Module) {
        let export_map = collect_exports_map(n);
        self.exports.extend(export_map);
    }

    fn add_leading_comment(&self, n: &Module) {
        if let Some(item) = n.body.first() {
            let mut comments = self.context.meta.script.origin_comments.write().unwrap();

            comments.add_leading_comment_at(
                item.span_lo(),
                Comment {
                    kind: CommentKind::Line,
                    span: DUMMY_SP,
                    text: format!(
                        " CONCATENATED MODULE: {}",
                        relative_to_root(&self.module_id.id, &self.context.root)
                    )
                    .into(),
                },
            );
        }
    }

    fn apply_renames(&mut self, n: &mut Module, export_map: &mut HashMap<String, ModuleRef>) {
        let map = self.rename_request.iter().cloned().collect();
        let mut renamer = IdentRenamer::new(&map);
        n.visit_mut_with(&mut renamer);

        // todo performance?
        for (from, to) in &map {
            self.exports.iter_mut().for_each(|(_k, v)| {
                if from.0.eq(v) {
                    *v = to.0.to_string();
                }
            });

            export_map.iter_mut().for_each(|(_k, v)| {
                if from.eq(&v.0.to_id()) {
                    v.0.sym = to.0.clone();
                }
            });
        }
    }

    fn get_non_conflict_name(&self, name: &str) -> String {
        self.concatenate_context
            .negotiate_safe_var_name(&self.my_top_decls, name)
    }

    fn resolve_conflict(&mut self) {
        let conflicts: Vec<_> = self
            .concatenate_context
            .top_level_vars
            .intersection(&self.my_top_decls)
            .cloned()
            .collect();

        let ctxt = SyntaxContext::empty().apply_mark(self.top_level_mark);

        for conflicted_name in conflicts {
            self.my_top_decls.remove(&conflicted_name);

            let new_name = self.get_non_conflict_name(&conflicted_name);

            self.exports
                .entry(conflicted_name.clone())
                .and_modify(|e| *e = new_name.clone());
            self.my_top_decls.insert(new_name.clone());
            self.rename_request
                .push(((conflicted_name.into(), ctxt), (new_name.into(), ctxt)));
        }
    }

    fn import_decl_to_replace_items(&mut self, import: &ImportDecl) -> Option<Vec<ModuleItem>> {
        let src = import.src.value.to_string();
        if let Some(src_module_id) = self.src_to_module.get(&src)
            && let Some(exports_map) = self.concatenate_context.modules_in_scope.get(src_module_id)
        {
            let stmts = import
                .specifiers
                .iter()
                .flat_map(|specifier| {
                    inner_import_specifier_to_stmts(&mut self.my_top_decls, specifier, exports_map)
                })
                .map(|s| s.into())
                .collect();
            Some(stmts)
        } else {
            None
        }
    }

    fn remove_current_stmt(&mut self) {
        self.replaces.push((self.current_stmt_index, vec![]));
    }

    fn replace_current_stmt_with(&mut self, stmts: Vec<ModuleItem>) {
        self.replaces.push((self.current_stmt_index, stmts));
    }
}

impl<'a> VisitMut for RootTransformer<'a> {
    fn visit_mut_export_decl(&mut self, export_decl: &mut ExportDecl) {
        let decl = export_decl.decl.take();

        let stmt: Stmt = decl.into();
        self.replaces
            .push((self.current_stmt_index, vec![stmt.into()]));
    }

    fn visit_mut_export_default_decl(&mut self, export_default_dcl: &mut ExportDefaultDecl) {
        let default_binding_name = self.default_bind_name.clone();

        match &mut export_default_dcl.decl {
            DefaultDecl::Class(class_expr) => {
                let stmt: Stmt = match &class_expr.ident {
                    None => {
                        self.exports
                            .insert("default".to_string(), default_binding_name.clone());
                        self.my_top_decls.insert(default_binding_name.clone());

                        class_expr
                            .take()
                            .into_var_decl(
                                VarDeclKind::Var,
                                quote_ident!(default_binding_name).into(),
                            )
                            .into()
                    }
                    Some(ident) => {
                        let export_default_ident = ident.clone();
                        self.exports
                            .insert("default".to_string(), export_default_ident.sym.to_string());
                        self.my_top_decls
                            .insert(export_default_ident.sym.to_string());

                        let class_decl = ClassDecl {
                            ident: ident.clone(),
                            declare: false,
                            class: class_expr.class.take(),
                        };

                        class_decl.into()
                    }
                };

                self.replace_current_stmt_with(vec![stmt.into()]);
            }
            DefaultDecl::Fn(default_fn_dcl) => {
                let stmt: Stmt = match &default_fn_dcl.ident {
                    None => {
                        let stmt: Stmt = default_fn_dcl
                            .take()
                            .into_var_decl(
                                VarDeclKind::Var,
                                quote_ident!(default_binding_name.clone()).into(),
                            )
                            .into();

                        self.exports
                            .insert("default".to_string(), default_binding_name.clone());
                        self.my_top_decls.insert(default_binding_name);
                        stmt
                    }
                    Some(fn_ident) => {
                        let default_binding_name = fn_ident.sym.to_string();

                        self.exports
                            .insert("default".to_string(), default_binding_name.clone());
                        self.my_top_decls.insert(default_binding_name);

                        let fn_decl = FnDecl {
                            ident: fn_ident.clone(),
                            declare: false,
                            function: default_fn_dcl.function.take(),
                        };

                        fn_decl.into()
                    }
                };

                self.replace_current_stmt_with(vec![stmt.into()]);
            }
            DefaultDecl::TsInterfaceDecl(_) => {
                unreachable!("TS should already be stripped")
            }
        }
    }

    fn visit_mut_export_default_expr(&mut self, export_default_expr: &mut ExportDefaultExpr) {
        let span = export_default_expr.span.apply_mark(self.top_level_mark);

        let default_binding_name = self.default_bind_name.clone();

        if let Some(exported_ident) = export_default_expr.expr.as_ident() {
            self.exports
                .insert("default".to_string(), exported_ident.sym.to_string());
            self.remove_current_stmt();
        } else {
            let stmt: Stmt = export_default_expr
                .expr
                .take()
                .into_var_decl(
                    VarDeclKind::Var,
                    quote_ident!(span, default_binding_name.clone()).into(),
                )
                .into();
            self.my_top_decls.insert(default_binding_name.clone());
            //TODO how to sync with export_ref_map
            self.exports
                .insert("default".to_string(), default_binding_name);
            self.replace_current_stmt_with(vec![stmt.into()]);
        }
    }

    fn visit_mut_import_decl(&mut self, _import_decl: &mut ImportDecl) {
        self.remove_current_stmt();
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        self.collect_exports(n);

        let mut var_links_collector = ModuleDeclMapCollector::new(self.default_bind_name.clone());
        n.visit_with(&mut var_links_collector);

        let ModuleDeclMapCollector {
            import_map,
            export_map,
            ..
        } = var_links_collector;

        let import_ref_map = self.to_import_module_ref(&import_map);
        let mut export_ref_map = self.to_export_module_ref(&export_map);

        // strip all the module declares
        n.visit_mut_children_with(self);

        let mut rewriter = ModuleRefRewriter::new(&import_ref_map, Default::default(), true);
        n.visit_mut_with(&mut rewriter);
        self.remove_imported_top_vars(&import_ref_map);

        self.resolve_conflict();
        self.apply_renames(n, &mut export_ref_map);
        self.add_leading_comment(n);

        self.concatenate_context
            .modules_in_scope
            .insert(self.module_id.clone(), self.exports.clone());

        self.concatenate_context
            .modules_exports_map
            .insert(self.module_id.clone(), export_ref_map);

        self.concatenate_context
            .top_level_vars
            .extend(self.my_top_decls.iter().cloned());
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        for index in (0..items.len()).rev() {
            self.current_stmt_index = index;
            let item = items.get_mut(index).unwrap();

            if let Some(module_decl) = item.as_mut_module_decl() {
                module_decl.visit_mut_with(self);
            }
        }

        for (index, rep) in self.replaces.take() {
            items.splice(index..index + 1, rep);
        }
    }

    fn visit_mut_named_export(&mut self, named_export: &mut NamedExport) {
        if let Some(export_src) = &named_export.src {
            if let Some(imported_module_id) = self.src_to_module.get(&export_src.value.to_string())
                && let Some(export_map) = self
                    .concatenate_context
                    .modules_in_scope
                    .get(imported_module_id)
            {
                let mut stmts: Vec<ModuleItem> = vec![];

                for spec in &named_export.specifiers {
                    match spec {
                        ExportSpecifier::Namespace(ns) => {
                            let exported_namespace = export_map.get("*").unwrap();

                            match &ns.name {
                                ModuleExportName::Ident(name_ident) => {
                                    self.exports.insert(
                                        name_ident.sym.to_string(),
                                        exported_namespace.clone(),
                                    );
                                }
                                ModuleExportName::Str(_) => {
                                    unimplemented!("{}", MODULE_CONCATENATE_ERROR_STR_MODULE_NAME);
                                }
                            }
                        }
                        ExportSpecifier::Default(_) => {
                            let default_export_name = export_map.get("default").unwrap();

                            let default_binding_name = self.get_non_conflict_name(
                                &uniq_module_default_export_name(self.module_id),
                            );

                            let stmt: Stmt = declare_var_with_init_stmt(
                                quote_ident!(default_binding_name.clone()),
                                default_export_name,
                            );
                            self.my_top_decls.insert(default_binding_name.clone());
                            self.exports
                                .insert("default".to_string(), default_binding_name);
                            stmts.push(stmt.into());
                        }
                        ExportSpecifier::Named(named) => {
                            let (exported_ident, orig_name) = match (&named.exported, &named.orig) {
                                (None, ModuleExportName::Ident(orig)) => {
                                    (orig.clone(), orig.sym.to_string())
                                }
                                (
                                    Some(ModuleExportName::Ident(exported_ident)),
                                    ModuleExportName::Ident(orig_ident),
                                ) => (exported_ident.clone(), orig_ident.sym.to_string()),
                                (_, _) => {
                                    unimplemented!("{}", MODULE_CONCATENATE_ERROR_STR_MODULE_NAME)
                                }
                            };

                            if let Some(mapped_export) = export_map.get(&orig_name) {
                                self.exports
                                    .insert(exported_ident.sym.to_string(), mapped_export.clone());
                            }
                        }
                    }
                }

                self.replaces.push((self.current_stmt_index, stmts));
            } else {
                // TODO handle export * from "external"
                self.remove_current_stmt();
            }
        } else {
            for export_spec in &named_export.specifiers {
                match export_spec {
                    ExportSpecifier::Namespace(_) => {
                        unreachable!("namespace export unreachable when no src")
                    }
                    ExportSpecifier::Default(_) => {
                        unreachable!("default export unreachable when no src")
                    }
                    ExportSpecifier::Named(named) => {
                        match (&named.exported, &named.orig) {
                            (
                                Some(ModuleExportName::Ident(exported_ident)),
                                ModuleExportName::Ident(orig_ident),
                            ) => {
                                self.exports.insert(
                                    exported_ident.sym.to_string(),
                                    orig_ident.sym.to_string(),
                                );
                            }
                            (None, ModuleExportName::Ident(_)) => {
                                // nothing to do
                                // export map already set as ident-ident
                                // module item it will be removed
                            }
                            (_, ModuleExportName::Str(_)) | (Some(ModuleExportName::Str(_)), _) => {
                                unimplemented!("export 'str' not supported now");
                            }
                        }
                    }
                }
            }

            self.remove_current_stmt();
        }
    }
}

pub fn inner_import_specifier_to_stmts(
    local_top_decls: &mut HashSet<String>,
    import_specifier: &ImportSpecifier,
    exports_map: &HashMap<String, String>,
) -> Vec<Stmt> {
    let mut stmts: Vec<Stmt> = vec![];

    // let mut rename_request = vec![];

    match &import_specifier {
        ImportSpecifier::Named(named_import) => {
            let imported_name = match &named_import.imported {
                None => named_import.local.sym.to_string(),
                Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
                Some(ModuleExportName::Str(_)) => {
                    unimplemented!("")
                }
            };

            let local = named_import.local.sym.to_string();

            if let Some(mapped_export) = exports_map.get(&imported_name) {
                if local != *mapped_export {
                    let stmt: Stmt =
                        declare_var_with_init_stmt(named_import.local.clone(), mapped_export);

                    stmts.push(stmt);
                } else {
                    local_top_decls.remove(&local);
                }
            }
        }
        ImportSpecifier::Default(default_import) => {
            if let Some(default_export_name) = exports_map.get("default") {
                if default_export_name.ne(default_import.local.sym.as_ref()) {
                    let stmt: Stmt = quote_ident!(default_export_name.clone())
                        .into_var_decl(VarDeclKind::Var, default_import.local.clone().into())
                        .into();

                    stmts.push(stmt);
                } else {
                    local_top_decls.remove(default_export_name);
                }
            }
        }
        ImportSpecifier::Namespace(namespace) => {
            let exported_namespace = exports_map.get("*").unwrap();

            if exported_namespace.ne(namespace.local.sym.as_ref()) {
                let stmt: Stmt = quote_ident!(exported_namespace.clone())
                    .into_var_decl(VarDeclKind::Var, namespace.local.clone().into())
                    .into();
                stmts.push(stmt);
            } else {
                local_top_decls.remove(exported_namespace);
            }
        }
    }

    stmts
}

fn export_named_specifier_to_orig_and_exported(named: &ExportNamedSpecifier) -> (Ident, String) {
    match (&named.exported, &named.orig) {
        (None, ModuleExportName::Ident(orig)) => (orig.clone(), orig.sym.to_string()),
        (Some(ModuleExportName::Ident(exported_ident)), ModuleExportName::Ident(orig_ident)) => {
            (exported_ident.clone(), orig_ident.sym.to_string())
        }
        (_, _) => {
            unimplemented!("{}", MODULE_CONCATENATE_ERROR_STR_MODULE_NAME)
        }
    }
}
