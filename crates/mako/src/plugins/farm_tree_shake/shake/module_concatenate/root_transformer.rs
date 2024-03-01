use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ExportNamedSpecifier, ExportSpecifier, Id, Ident, KeyValueProp, Module, ModuleDecl,
    ModuleExportName, ModuleItem, NamedExport, ObjectLit, Prop, PropOrSpread, Stmt,
};
use swc_core::ecma::utils::{
    collect_decls_with_ctxt, member_expr, quote_ident, ExprFactory, IdentRenamer,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::{relative_to_root, ModuleId};
use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::ConcatenateContext;
use crate::plugins::farm_tree_shake::shake::module_concatenate::inner_transformer::inner_import_specifier_to_stmts;
use crate::plugins::farm_tree_shake::shake::module_concatenate::utils::MODULE_CONCATENATE_ERROR_STR_MODULE_NAME;

pub(super) struct RootTransformer<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub current_module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
    pub top_level_mark: Mark,
    pub import_source_to_module_id: &'a HashMap<String, ModuleId>,
    my_top_decls: HashSet<String>,
}

impl RootTransformer<'_> {
    pub fn new<'a>(
        concatenate_context: &'a mut ConcatenateContext,
        current_module_id: &'a ModuleId,
        context: &'a Arc<Context>,
        top_level_mark: Mark,
        import_source_to_module_id: &'a HashMap<String, ModuleId>,
    ) -> RootTransformer<'a> {
        RootTransformer {
            concatenate_context,
            current_module_id,
            context,
            top_level_mark,
            import_source_to_module_id,
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

    fn resolve_conflicts(&mut self, n: &mut Module) {
        let mut my_top_decls =
            collect_decls_with_ctxt(n, SyntaxContext::empty().apply_mark(self.top_level_mark))
                .iter()
                .map(|id: &Id| id.0.to_string())
                .collect();

        let conflicts_idents: HashSet<_> = self
            .concatenate_context
            .top_level_vars
            .intersection(&my_top_decls)
            .cloned()
            .collect();

        let mut map: Vec<(Id, Id)> = Default::default();
        let syntax = SyntaxContext::empty().apply_mark(self.top_level_mark);
        for conflict in conflicts_idents {
            let new_name_base = format!("__{}", conflict);
            let new_name = self
                .concatenate_context
                .negotiate_safe_var_name(&self.my_top_decls, &new_name_base);

            my_top_decls.insert(new_name.clone());
            map.push(((conflict.clone().into(), syntax), (new_name.into(), syntax)));
        }

        let map2 = map.iter().cloned().collect();

        let mut rename = IdentRenamer::new(&map2);
        n.visit_mut_with(&mut rename);
    }

    fn src_exports(&self, src: &str) -> Option<&HashMap<String, String>> {
        self.import_source_to_module_id
            .get(src)
            .and_then(|module_id| self.concatenate_context.modules_in_scope.get(module_id))
    }
}

impl<'a> VisitMut for RootTransformer<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        self.add_comment(n);

        let mut replaces = vec![];
        for (index, module_item) in n.body.iter().enumerate().rev() {
            if let Some(module_decl) = module_item.as_module_decl() {
                let mut items: Option<Vec<ModuleItem>> = None;

                match module_decl {
                    ModuleDecl::Import(import_decl) => {
                        items = if let Some(src_module_id) = self
                            .import_source_to_module_id
                            .get(import_decl.src.value.as_ref())
                            && let Some(exports_map) =
                                self.concatenate_context.modules_in_scope.get(src_module_id)
                        {
                            Some(
                                import_decl
                                    .specifiers
                                    .iter()
                                    .flat_map(|spec| {
                                        inner_import_specifier_to_stmts(
                                            &mut self.my_top_decls,
                                            spec,
                                            exports_map,
                                        )
                                    })
                                    .map(|st| st.into())
                                    .collect(),
                            )
                        } else {
                            None
                        };
                    }
                    ModuleDecl::ExportDecl(_) => {}
                    ModuleDecl::ExportNamed(export_named) => {
                        if let Some(imported_src) = &export_named.src {
                            if let Some(export_map) = self.src_exports(imported_src.value.as_ref())
                            {
                                items = Some(vec![]);

                                for spec in &export_named.specifiers {
                                    match spec {
                                        ExportSpecifier::Namespace(_) => {
                                            todo!();
                                        }
                                        ExportSpecifier::Default(_) => {
                                            todo!();
                                        }
                                        ExportSpecifier::Named(named) => {
                                            let (exported_ident, orig) =
                                                export_named_specifier_to_orig_and_exported(named);

                                            if let Some(mapped_export) = export_map.get(&orig) {
                                                let i = items.as_mut().unwrap();

                                                if exported_ident.sym.eq(mapped_export) {
                                                    let module_dcl: ModuleDecl = NamedExport {
                                                        span: Default::default(),
                                                        specifiers: vec![ExportNamedSpecifier {
                                                            span: Default::default(),
                                                            orig: ModuleExportName::Ident(
                                                                exported_ident.clone(),
                                                            ),
                                                            exported: None,
                                                            is_type_only: false,
                                                        }
                                                        .into()],
                                                        type_only: false,
                                                        src: None,
                                                        with: None,
                                                    }
                                                    .into();

                                                    i.push(module_dcl.into());
                                                } else {
                                                    let export_decl: ModuleDecl = NamedExport {
                                                        span: Default::default(),
                                                        specifiers: vec![ExportNamedSpecifier {
                                                            span: Default::default(),
                                                            orig: ModuleExportName::Ident(
                                                                quote_ident!(mapped_export.clone()),
                                                            ),
                                                            exported: Some(
                                                                ModuleExportName::Ident(
                                                                    exported_ident,
                                                                ),
                                                            ),
                                                            is_type_only: false,
                                                        }
                                                        .into()],
                                                        src: None,
                                                        type_only: false,
                                                        with: None,
                                                    }
                                                    .into();

                                                    i.push(export_decl.into());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportDefaultDecl(_) => {}
                    ModuleDecl::ExportDefaultExpr(_) => {}
                    ModuleDecl::ExportAll(export_all) => {
                        if let Some(module_exports) =
                            self.src_exports(export_all.src.value.as_ref())
                        {
                            let mut key_value_props: Vec<PropOrSpread> = vec![];
                            for (exported_name, local_name) in module_exports.iter() {
                                if exported_name.ne("default") {
                                    key_value_props.push(
                                        Prop::KeyValue(KeyValueProp {
                                            key: quote_ident!(exported_name.clone()).into(),
                                            value: quote_ident!(local_name.clone())
                                                .into_lazy_fn(vec![])
                                                .into(),
                                        })
                                        .into(),
                                    )
                                }
                            }

                            let define_exports: Stmt = member_expr!(DUMMY_SP, __mako_require__.e)
                                .as_call(
                                    DUMMY_SP,
                                    vec![
                                        quote_ident!("exports").as_arg(),
                                        ObjectLit {
                                            span: DUMMY_SP,
                                            props: key_value_props,
                                        }
                                        .as_arg(),
                                    ],
                                )
                                .into_stmt();

                            items = Some(vec![define_exports.into()]);
                        }
                    }
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }

                if let Some(items) = items {
                    replaces.push((index, items));
                }
            }
        }

        for (i, items) in replaces {
            n.body.splice(i..i + 1, items);
        }

        self.resolve_conflicts(n);
    }
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
