use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::swc_common::util::take::Take;
use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    DefaultDecl, ExportSpecifier, Id, ImportSpecifier, KeyValueProp, Module, ModuleDecl,
    ModuleExportName, ModuleItem, ObjectLit, Prop, PropOrSpread, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{
    collect_decls_with_ctxt, member_expr, quote_ident, ExprFactory, IdentRenamer,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use super::exports_transform::collect_exports_map;
use super::utils::uniq_module_prefix;
use crate::compiler::Context;
use crate::module::{relative_to_root, ImportType, ModuleId};

pub(super) struct InnerTransform<'a> {
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,
    pub modules_in_scope: &'a mut HashMap<ModuleId, HashMap<String, String>>,
    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub current_top_level_vars: &'a mut HashSet<String>,
    pub top_level_mark: Mark,

    my_top_decls: HashSet<String>,
    exports: HashMap<String, String>,
    rename_request: Vec<(Id, Id)>,
    imported_type: ImportType,
}

impl<'a> InnerTransform<'a> {
    pub fn new<'s>(
        modules_in_scope: &'a mut HashMap<ModuleId, HashMap<String, String>>,
        top_level_var: &'a mut HashSet<String>,
        module_id: &'a ModuleId,
        src_to_module: &'a HashMap<String, ModuleId>,
        context: &'a Arc<Context>,
        top_level_mark: Mark,
    ) -> InnerTransform<'a>
    where
        'a: 's,
    {
        Self {
            modules_in_scope,
            current_top_level_vars: top_level_var,
            module_id,
            src_to_module,
            context,
            top_level_mark,
            exports: Default::default(),
            my_top_decls: Default::default(),
            rename_request: vec![],
            imported_type: Default::default(),
        }
    }

    pub fn imported(&mut self, imported_type: ImportType) {
        self.imported_type = imported_type;
    }

    fn collect_exports(&mut self, n: &Module) {
        let mut export_map = collect_exports_map(n);
        if export_map.get(&"default".to_string()).is_some() {
            let default_name = uniq_module_default_export_name(self.module_id, self.context);

            export_map.insert("default".to_string(), default_name.clone());
            self.my_top_decls.insert(default_name);
        }

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

    fn apply_renames(&mut self, n: &mut Module) {
        let map = self.rename_request.iter().cloned().collect();
        let mut renamer = IdentRenamer::new(&map);
        n.visit_mut_with(&mut renamer);
    }

    fn get_non_conflict_name(&self, name: &String) -> String {
        let mut new_name = name.to_string();
        let mut i = 0;
        while self.current_top_level_vars.contains(&new_name)
            || self.my_top_decls.contains(&new_name)
        {
            new_name = format!("{}_{}", name, i);
            i += 1;
        }
        new_name
    }

    fn resolve_conflict(&mut self) {
        let conflicts: Vec<_> = self
            .current_top_level_vars
            .intersection(&self.my_top_decls)
            .cloned()
            .collect();

        let ctxt = SyntaxContext::empty().apply_mark(self.top_level_mark);
        let default_name = uniq_module_default_export_name(self.module_id, self.context);
        for conflicted_name in conflicts {
            self.my_top_decls.remove(&conflicted_name);

            let new_name = self.get_non_conflict_name(&conflicted_name);

            if conflicted_name == default_name {
                self.exports.insert("default".to_string(), new_name.clone());
            }

            self.exports
                .entry(conflicted_name.clone())
                .and_modify(|e| *e = new_name.clone());
            self.my_top_decls.insert(new_name.clone());
            self.rename_request
                .push(((conflicted_name.into(), ctxt), (new_name.into(), ctxt)));
        }
    }
}

impl<'a> VisitMut for InnerTransform<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        self.my_top_decls =
            collect_decls_with_ctxt(n, SyntaxContext::empty().apply_mark(self.top_level_mark))
                .iter()
                .map(|id: &Id| id.0.to_string())
                .collect();
        self.collect_exports(n);

        n.visit_mut_children_with(self);

        self.resolve_conflict();
        self.apply_renames(n);
        self.add_leading_comment(n);

        if (self.imported_type & ImportType::Namespace) == ImportType::Namespace {
            let ns_name = self
                .get_non_conflict_name(&uniq_module_namespace_name(self.module_id, self.context));
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
            self.my_top_decls.insert(ns_name);

            n.body.push(init_stmt.into());
            n.body.push(define_exports.into());
        }

        self.modules_in_scope
            .insert(self.module_id.clone(), self.exports.clone());
        self.current_top_level_vars
            .extend(self.my_top_decls.iter().cloned());
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        let mut replaces = vec![];

        for index in (0..items.len()).rev() {
            let mut stmts: Option<Vec<ModuleItem>> = None;

            let item = items.get_mut(index).unwrap();

            if let Some(module_decl) = item.as_mut_module_decl() {
                match module_decl {
                    ModuleDecl::Import(import) => {
                        let src = import.src.value.to_string();
                        stmts = Some(vec![]);

                        if let Some(src_module_id) = self.src_to_module.get(&src)
                            && let Some(exports_map) = self.modules_in_scope.get(src_module_id)
                        {
                            for import_specifier in import.specifiers.iter() {
                                match &import_specifier {
                                    ImportSpecifier::Named(named_import) => {
                                        if let Some(imported) = &named_import.imported {
                                            let imported_name = match &imported {
                                                ModuleExportName::Ident(id) => id.sym.to_string(),
                                                ModuleExportName::Str(str) => str.value.to_string(),
                                            };

                                            let local = named_import.local.sym.to_string();

                                            self.my_top_decls.remove(&local);

                                            if let Some(mapped_export) =
                                                exports_map.get(&imported_name)
                                            {
                                                if local != *mapped_export {
                                                    self.rename_request.push((
                                                        Id::from(named_import.local.clone()),
                                                        (
                                                            mapped_export.clone().into(),
                                                            named_import.local.span.ctxt,
                                                        ),
                                                    ));
                                                }
                                            }
                                        } else {
                                            let local = named_import.local.sym.to_string();

                                            self.my_top_decls.remove(&local);

                                            if let Some(mapped_export) = exports_map.get(&local) {
                                                if *mapped_export != local {
                                                    self.rename_request.push((
                                                        Id::from(named_import.local.clone()),
                                                        (
                                                            mapped_export.clone().into(),
                                                            named_import.local.span.ctxt,
                                                        ),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    ImportSpecifier::Default(default_import) => {
                                        if let Some(default_export_name) =
                                            exports_map.get("default")
                                        {
                                            let stmt: Stmt =
                                                quote_ident!(default_export_name.clone())
                                                    .into_var_decl(
                                                        VarDeclKind::Var,
                                                        default_import.local.clone().into(),
                                                    )
                                                    .into();

                                            stmts.as_mut().unwrap().push(stmt.into());
                                        }
                                    }
                                    ImportSpecifier::Namespace(_) => {
                                        // should never here
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportDecl(export_decl) => {
                        let decl = export_decl.decl.take();

                        let stmt: Stmt = decl.into();

                        *item = stmt.into();
                    }
                    ModuleDecl::ExportNamed(named_export) => {
                        if let Some(export_src) = &named_export.src {
                            let msg = format!(
                                "export from {:?} not supported in inner module({}) yet",
                                export_src.value, self.module_id.id
                            );
                            todo!("{}", msg);
                        } else {
                            let mut dcl_stmts: Vec<ModuleItem> = vec![];

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
                                                let dccl = exported_ident.clone().into_var_decl(
                                                    VarDeclKind::Var,
                                                    orig_ident.clone().into(),
                                                );
                                                dcl_stmts.push(dccl.into());
                                            }
                                            (None, ModuleExportName::Ident(_)) => {
                                                // do nothing, it will be removed
                                            }
                                            (_, ModuleExportName::Str(_))
                                            | (Some(ModuleExportName::Str(_)), _) => {
                                                unimplemented!("export 'str' not supported now");
                                            }
                                        }
                                    }
                                }
                            }

                            stmts = Some(dcl_stmts);
                        }
                    }
                    ModuleDecl::ExportDefaultDecl(export_default_dcl) => {
                        match &mut export_default_dcl.decl {
                            DefaultDecl::Class(dcl) => {
                                let stmt: Stmt = dcl
                                    .take()
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        quote_ident!(uniq_module_default_export_name(
                                            self.module_id,
                                            self.context
                                        ))
                                        .into(),
                                    )
                                    .into();

                                items[index] = stmt.into();
                            }
                            DefaultDecl::Fn(dcl) => {
                                let stmt: Stmt = dcl
                                    .take()
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        quote_ident!(uniq_module_default_export_name(
                                            self.module_id,
                                            self.context
                                        ))
                                        .into(),
                                    )
                                    .into();

                                items[index] = stmt.into();
                            }
                            DefaultDecl::TsInterfaceDecl(_) => {
                                unreachable!("TS should already be stripped")
                            }
                        }
                    }
                    ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                        let span = export_default_expr.span.apply_mark(self.top_level_mark);

                        let stmt: Stmt = export_default_expr
                            .expr
                            .take()
                            .into_var_decl(
                                VarDeclKind::Const,
                                quote_ident!(
                                    span,
                                    uniq_module_default_export_name(self.module_id, self.context)
                                )
                                .into(),
                            )
                            .into();

                        *item = stmt.into();
                    }
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }
            }

            if let Some(to_replace) = stmts {
                replaces.push((index, to_replace));
            }
        }

        for (index, rep) in replaces {
            items.splice(index..index + 1, rep);
        }
    }
}

fn uniq_module_default_export_name(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!("{}_0", uniq_module_prefix(module_id, context))
}

fn uniq_module_namespace_name(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!("{}_ns", uniq_module_prefix(module_id, context))
}
