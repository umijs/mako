use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use swc_core::common::collections::AHashSet;
use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::util::take::Take;
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ClassDecl, DefaultDecl, ExportAll, ExportDecl, ExportDefaultDecl, ExportDefaultExpr, FnDecl,
    Id, ImportDecl, KeyValueProp, Module, ModuleItem, NamedExport, ObjectLit, Prop, PropOrSpread,
    Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{member_expr, quote_ident, ExprFactory, IdentRenamer};
use swc_core::ecma::visit::{VisitMut, VisitMutWith, VisitWith};

use super::concatenate_context::{
    all_referenced_variables, module_ref_to_expr, ConcatenateContext, ImportModuleRefMap, ModuleRef,
};
use super::module_ref_rewriter::ModuleRefRewriter;
use super::ref_link::{ModuleDeclMapCollector, Symbol, VarLink};
use super::utils::{uniq_module_default_export_name, uniq_module_namespace_name};
use crate::compiler::Context;
use crate::module::{relative_to_root, ImportType, ModuleId};

pub enum InnerOrExternal<'a> {
    Inner(&'a HashMap<String, ModuleRef>),
    External(&'a (String, String)),
}

pub(super) struct ConcatenatedTransform<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,

    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub top_level_mark: Mark,

    my_top_decls: HashSet<String>,
    all_decls: AHashSet<Id>,
    rename_request: Vec<(Id, Id)>,
    imported_type: ImportType,
    current_stmt_index: usize,
    replaces: Vec<(usize, Vec<ModuleItem>)>,
    default_bind_name: String,
    is_root: bool,
}

impl<'a> ConcatenatedTransform<'a> {
    pub fn new(
        concatenate_context: &'a mut ConcatenateContext,
        module_id: &'a ModuleId,
        src_to_module: &'a HashMap<String, ModuleId>,
        context: &'a Arc<Context>,
        top_level_mark: Mark,
    ) -> Self {
        Self {
            concatenate_context,
            module_id,
            src_to_module,
            context,
            top_level_mark,
            my_top_decls: Default::default(),
            all_decls: Default::default(),
            rename_request: vec![],
            imported_type: Default::default(),
            replaces: vec![],
            current_stmt_index: 0,
            default_bind_name: Default::default(),
            is_root: false,
        }
    }

    pub fn for_root(mut self) -> Self {
        self.is_root = true;
        self
    }

    fn add_leading_comment(&self, n: &Module) {
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
                        " CONCATENATED MODULE: {}",
                        relative_to_root(&self.module_id.id, &self.context.root)
                    )
                    .into(),
                    span: DUMMY_SP,
                },
            );
        }
    }

    pub(crate) fn to_import_module_ref(
        &self,
        var_map: &HashMap<Id, VarLink>,
    ) -> ImportModuleRefMap {
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
                            Symbol::Default => {
                                if let Some(mf) = map.get("default") {
                                    mf.clone()
                                } else {
                                    (quote_ident!("undefined"), None)
                                }
                            }
                            Symbol::Namespace => map.get("*").unwrap().clone(),
                            Symbol::Var(ident) => {
                                if let Some(mf) = map.get(&ident.sym.to_string()) {
                                    mf.clone()
                                } else {
                                    (quote_ident!("undefined"), None)
                                }
                            }
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
        let mut expanded_export_all = vec![];

        var_map.iter().for_each(|(id, link)| match link {
            VarLink::Direct(direct_id) => {
                ref_map.insert(id.0.to_string(), (direct_id.clone().into(), None));
            }
            VarLink::InDirect(symbol, source) => {
                let src_module_id = self.src_to_module.get(source).unwrap();

                match self.inner_or_external(src_module_id) {
                    InnerOrExternal::Inner(map) => {
                        let module_ref = match symbol {
                            Symbol::Default => {
                                if let Some(mf) = map.get("default") {
                                    mf.clone()
                                } else {
                                    (quote_ident!("undefined"), None)
                                }
                            }
                            Symbol::Namespace => map.get("*").unwrap().clone(),
                            Symbol::Var(ident) => {
                                if let Some(mf) = map.get(&ident.sym.to_string()) {
                                    mf.clone()
                                } else {
                                    (quote_ident!("undefined"), None)
                                }
                            }
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
            VarLink::All(source, order) => {
                if let Some(src_module_id) = self.src_to_module.get(source) {
                    if let Some(map) = self
                        .concatenate_context
                        .modules_exports_map
                        .get(src_module_id)
                    {
                        map.iter().for_each(|(k, v)| {
                            if k != "default" && k != "*" {
                                expanded_export_all.push((order, (k.clone(), v.clone())))
                            }
                        });
                    }
                }
                // else it's export * from external module, it only happens in root so will be
                // handled in root
            }
        });
        expanded_export_all.sort_by_key(|(order, _)| *order);

        for (_, (k, v)) in expanded_export_all {
            ref_map.entry(k.clone()).or_insert_with(|| v.clone());
        }
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

    fn remove_imported_top_vars(&mut self, import_map: &ImportModuleRefMap) {
        import_map.iter().for_each(|(id, _)| {
            self.my_top_decls.remove(&id.0.to_string());
            self.all_decls.remove(id);
        });
    }

    fn inner_or_external(&self, module_id: &ModuleId) -> InnerOrExternal {
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

    fn apply_renames(&mut self, n: &mut Module, export_map: &mut HashMap<String, ModuleRef>) {
        let map = self.rename_request.iter().cloned().collect();
        let mut renamer = IdentRenamer::new(&map);
        n.visit_mut_with(&mut renamer);

        // todo performance?
        for (from, to) in &map {
            export_map.iter_mut().for_each(|(_k, v)| {
                if from.eq(&v.0.to_id()) {
                    v.0.sym = to.0.clone();
                }
            });
        }
    }

    fn get_non_conflict_top_level_name(&self, name: &str) -> String {
        self.concatenate_context
            .negotiate_safe_var_name(&self.my_top_decls, name)
    }

    fn negotiate_var_name_with(&self, reserved: &HashSet<String>, name: &str) -> String {
        self.concatenate_context
            .negotiate_safe_var_name(reserved, name)
    }

    fn resolve_conflict(&mut self, import_module_ref: &ImportModuleRefMap) {
        let top_ctxt = SyntaxContext::empty().apply_mark(self.top_level_mark);

        let imported_reference = all_referenced_variables(import_module_ref);

        let all_syms = self
            .all_decls
            .iter()
            .map(|(sym, _)| sym.to_string())
            .collect::<HashSet<_>>();

        for id in &self.all_decls {
            if id.1 == top_ctxt {
                if self
                    .concatenate_context
                    .top_level_vars
                    .contains(id.0.as_ref())
                {
                    let conflicted = id.0.as_ref();

                    let new_name = self.get_non_conflict_top_level_name(conflicted);

                    self.my_top_decls.remove(conflicted);
                    self.my_top_decls.insert(new_name.clone());

                    self.rename_request
                        .push((id.clone(), (new_name.into(), id.1)));
                }
            } else if imported_reference.contains(id.0.as_ref()) {
                let conflicted = id.0.as_ref();
                let new_name = self.negotiate_var_name_with(&all_syms, conflicted);
                self.rename_request
                    .push((id.clone(), (new_name.into(), id.1)));
            }
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
        let ns_name =
            self.get_non_conflict_top_level_name(&uniq_module_namespace_name(self.module_id));
        let ns_ident = quote_ident!(ns_name.clone());

        let empty_obj = ObjectLit {
            span: DUMMY_SP,
            props: vec![],
        };

        let init_stmt: Stmt = empty_obj
            .into_var_decl(VarDeclKind::Var, ns_ident.clone().into())
            .into();

        let mut key_value_props: Vec<KeyValueProp> = vec![];

        for (k, module_ref) in &mut *export_ref_map {
            key_value_props.push(KeyValueProp {
                key: quote_ident!(k.clone()).into(),
                value: module_ref_to_expr(module_ref).into_lazy_fn(vec![]).into(),
            });
        }

        key_value_props.sort_by_key(|prop| prop.key.as_ident().unwrap().sym.to_string());
        let key_value_props = key_value_props
            .into_iter()
            .map(Prop::KeyValue)
            .map(Into::into)
            .collect::<Vec<PropOrSpread>>();

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

        export_ref_map.insert("*".to_string(), (quote_ident!(ns_name.clone()), None));
        self.my_top_decls.insert(ns_name);

        n.body.push(init_stmt.into());
        n.body.push(define_exports.into());
    }
}

impl<'a> VisitMut for ConcatenatedTransform<'a> {
    fn visit_mut_export_all(&mut self, _export_all: &mut ExportAll) {
        self.remove_current_stmt();
    }

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

                        self.my_top_decls.insert(default_binding_name);
                        stmt
                    }
                    Some(fn_ident) => {
                        let default_binding_name = fn_ident.sym.to_string();

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

        if let Some(_exported_ident) = export_default_expr.expr.as_ident() {
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

            self.replace_current_stmt_with(vec![stmt.into()]);
        }
    }

    fn visit_mut_import_decl(&mut self, _import_decl: &mut ImportDecl) {
        self.remove_current_stmt();
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        // all root top vars is already in ccn context top vars
        self.my_top_decls = ConcatenateContext::top_level_vars(n, self.top_level_mark);
        self.all_decls = ConcatenateContext::all_decls(n);

        self.default_bind_name =
            self.get_non_conflict_top_level_name(&uniq_module_default_export_name(self.module_id));

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

        self.resolve_conflict(&import_ref_map);
        self.apply_renames(n, &mut export_ref_map);
        self.add_leading_comment(n);

        if self.imported_type.contains(ImportType::Namespace) {
            self.append_namespace_declare(n, &mut export_ref_map);
        }

        self.concatenate_context
            .modules_exports_map
            .insert(self.module_id.clone(), export_ref_map);

        self.concatenate_context
            .top_level_vars
            .extend(self.my_top_decls.iter().cloned());
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        // pay attention to the `rev()`
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

    fn visit_mut_named_export(&mut self, _: &mut NamedExport) {
        self.remove_current_stmt();
    }
}

impl<'a> ConcatenatedTransform<'a> {}

#[cfg(test)]
mod external_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod utils;
