use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::swc_common::util::take::Take;
use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ClassDecl, DefaultDecl, ExportDecl, ExportDefaultDecl, ExportDefaultExpr, ExportSpecifier,
    FnDecl, Id, ImportDecl, KeyValueProp, Module, ModuleExportName, ModuleItem, NamedExport,
    ObjectLit, Prop, PropOrSpread, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{member_expr, quote_ident, ExprFactory, IdentRenamer};
use swc_core::ecma::visit::{VisitMut, VisitMutWith, VisitWith};

use super::concatenate_context::{ConcatenateContext, ModuleRef, ModuleRefMap};
use super::exports_transform::collect_exports_map;
use super::module_ref_rewriter::ModuleRefRewriter;
use super::ref_link::{ModuleDeclMapCollector, Symbol, VarLink};
use super::utils::{
    declare_var_with_init_stmt, uniq_module_default_export_name, uniq_module_namespace_name,
    MODULE_CONCATENATE_ERROR_STR_MODULE_NAME,
};
use crate::compiler::Context;
use crate::module::{relative_to_root, ImportType, ModuleId};

pub(super) struct InnerTransform<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,

    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub top_level_mark: Mark,

    my_top_decls: HashSet<String>,
    exports: HashMap<String, String>,
    rename_request: Vec<(Id, Id)>,
    imported_type: ImportType,
    current_stmt_index: usize,
    replaces: Vec<(usize, Vec<ModuleItem>)>,
    default_bind_name: String,
}

pub enum InnerOrExternal<'a> {
    Inner(&'a HashMap<String, ModuleRef>),
    External(&'a (String, String)),
}

impl<'a> InnerTransform<'a> {
    pub fn new<'s>(
        concatenate_context: &'a mut ConcatenateContext,
        module_id: &'a ModuleId,
        src_to_module: &'a HashMap<String, ModuleId>,
        context: &'a Arc<Context>,
        top_level_mark: Mark,
    ) -> InnerTransform<'a>
    where
        'a: 's,
    {
        Self {
            concatenate_context,
            module_id,
            src_to_module,
            context,
            top_level_mark,
            exports: Default::default(),
            my_top_decls: Default::default(),
            rename_request: vec![],
            imported_type: Default::default(),
            replaces: vec![],
            current_stmt_index: 0,
            default_bind_name: Default::default(),
        }
    }

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
            VarLink::All(_, _) => {
                // never happens, there is no import * from "mod"
            }
        });

        ref_map
    }

    fn var_link_to_module_ref(
        &self,
        ref_map: &mut HashMap<String, ModuleRef>,
        var_map: &HashMap<&Id, &VarLink>,
    ) {
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
            VarLink::All(source, _) => {
                if let Some(src_module_id) = self.src_to_module.get(source) {
                    dbg!(&self.concatenate_context.modules_exports_map);

                    if let Some(map) = self
                        .concatenate_context
                        .modules_exports_map
                        .get(src_module_id)
                    {
                        map.iter().for_each(|(k, v)| {
                            if k != "default" && k != "*" {
                                ref_map.insert(k.clone(), v.clone());
                            }
                        });
                    }
                }
                // else it's export * from external module, it only happens in root so will be
                // handled in root
            }
        })
    }

    pub(crate) fn to_export_module_ref(
        &self,
        var_map: &HashMap<Id, VarLink>,
    ) -> HashMap<String, ModuleRef> {
        let mut ref_map = HashMap::new();

        let (export_all, normal): (HashMap<_, _>, HashMap<_, _>) = var_map
            .iter()
            .partition(|&(_, v)| matches!(v, VarLink::All(_, _)));

        self.var_link_to_module_ref(&mut ref_map, &normal);
        self.var_link_to_module_ref(&mut ref_map, &export_all);

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

    pub fn imported(&mut self, imported_type: ImportType) {
        self.imported_type = imported_type;
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

    fn remove_current_stmt(&mut self) {
        self.replaces.push((self.current_stmt_index, vec![]));
    }

    fn replace_current_stmt_with(&mut self, stmts: Vec<ModuleItem>) {
        self.replaces.push((self.current_stmt_index, stmts));
    }

    fn append_namespace_declare(
        &mut self,
        n: &mut Module,
        export_ref_map: &mut HashMap<String, ModuleRef>,
    ) {
        let ns_name = self.get_non_conflict_name(&uniq_module_namespace_name(self.module_id));
        let ns_ident = quote_ident!(ns_name.clone());

        let empty_obj = ObjectLit {
            span: DUMMY_SP,
            props: vec![],
        };

        let init_stmt: Stmt = empty_obj
            .into_var_decl(VarDeclKind::Var, ns_ident.clone().into())
            .into();

        let mut key_value_props: Vec<PropOrSpread> = vec![];

        for (exported_name, local_name) in self.exports.iter() {
            key_value_props.push(
                Prop::KeyValue(KeyValueProp {
                    key: quote_ident!(exported_name.clone()).into(),
                    value: quote_ident!(local_name.clone()).into_lazy_fn(vec![]).into(),
                })
                .into(),
            )
        }

        let define_exports: Stmt = member_expr!(DUMMY_SP, __mako_require__.e)
            .as_call(
                DUMMY_SP,
                vec![
                    ns_ident.as_arg(),
                    ObjectLit {
                        span: DUMMY_SP,
                        props: key_value_props,
                    }
                    .as_arg(),
                ],
            )
            .into_stmt();

        self.exports.insert("*".to_string(), ns_name.clone());
        export_ref_map.insert("*".to_string(), (quote_ident!(ns_name.clone()), None));
        self.my_top_decls.insert(ns_name);

        n.body.push(init_stmt.into());
        n.body.push(define_exports.into());
    }
}

impl<'a> VisitMut for InnerTransform<'a> {
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
        self.my_top_decls = ConcatenateContext::top_level_vars(n, self.top_level_mark);

        self.collect_exports(n);

        self.default_bind_name =
            self.get_non_conflict_name(&uniq_module_default_export_name(self.module_id));

        let mut var_links_collector = ModuleDeclMapCollector::new(self.default_bind_name.clone());
        n.visit_with(&mut var_links_collector);

        let ModuleDeclMapCollector {
            import_map,
            export_map,
            ..
        } = var_links_collector;

        let _import_ref_map = self.to_import_module_ref(&import_map);
        let mut export_ref_map = self.to_export_module_ref(&export_map);

        // strip all the module declares
        n.visit_mut_children_with(self);

        let mut rewriter = ModuleRefRewriter::new(&_import_ref_map, Default::default(), true);
        n.visit_mut_with(&mut rewriter);
        self.remove_imported_top_vars(&_import_ref_map);

        self.resolve_conflict();
        self.apply_renames(n, &mut export_ref_map);
        self.add_leading_comment(n);

        if self.imported_type.contains(ImportType::Namespace) {
            self.append_namespace_declare(n, &mut export_ref_map);
        }

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

impl<'a> InnerTransform<'a> {}

#[cfg(test)]
mod external_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod utils;
