use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    Decl, ExportDecl, ExportNamedSpecifier, ExportSpecifier, Id, Ident, ImportSpecifier, Module,
    ModuleDecl, ModuleExportName, ModuleItem, NamedExport, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{collect_decls_with_ctxt, quote_ident, ExprFactory, IdentRenamer};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::{relative_to_root, ModuleId};
use crate::plugins::farm_tree_shake::shake::module_concatenate::utils::{
    declare_var_with_init, MODULE_CONCATENATE_ERROR_STR_MODULE_NAME,
};

pub(super) struct RootTransformer<'a> {
    pub current_module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
    pub modules_in_scope: &'a HashMap<ModuleId, HashMap<String, String>>,
    pub top_level_vars: &'a HashSet<String>,
    pub top_level_mark: Mark,
    pub import_source_to_module_id: &'a HashMap<String, ModuleId>,
    pub renames: Vec<(Id, Id)>,
    my_top_decls: HashSet<String>,
}

impl RootTransformer<'_> {
    pub fn new<'a>(
        current_module_id: &'a ModuleId,
        context: &'a Arc<Context>,
        modules_in_scope: &'a HashMap<ModuleId, HashMap<String, String>>,
        top_level_vars: &'a HashSet<String>,
        top_level_mark: Mark,
        import_source_to_module_id: &'a HashMap<String, ModuleId>,
    ) -> RootTransformer<'a> {
        RootTransformer {
            current_module_id,
            context,
            modules_in_scope,
            top_level_vars,
            top_level_mark,
            import_source_to_module_id,
            renames: vec![],
            my_top_decls: HashSet::new(),
        }
    }

    fn request_rename(&mut self, req: (Id, Id)) {
        self.renames.push(req);
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
            .top_level_vars
            .intersection(&my_top_decls)
            .cloned()
            .collect();

        let mut map: Vec<(Id, Id)> = Default::default();
        let syntax = SyntaxContext::empty().apply_mark(self.top_level_mark);
        for conflict in conflicts_idents {
            let new_name_base = format!("__{}", conflict);

            let mut post_fix = 0;
            let mut new_name = format!("{}_{}", new_name_base, post_fix);
            while self.top_level_vars.contains(&new_name) || self.my_top_decls.contains(&new_name) {
                post_fix += 1;
                new_name = format!("{}_{}", new_name_base, post_fix);
            }

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
            .and_then(|module_id| self.modules_in_scope.get(module_id))
    }
}

impl<'a> VisitMut for RootTransformer<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        self.add_comment(n);

        let mut replaces = vec![];
        for (index, module_item) in n.body.iter().enumerate().rev() {
            if let Some(module_dc) = module_item.as_module_decl() {
                let mut items: Option<Vec<ModuleItem>> = None;

                match module_dc {
                    ModuleDecl::Import(import_decl) => {
                        let source = import_decl.src.value.to_string();

                        if let Some(imported_module_id) =
                            self.import_source_to_module_id.get(&source)
                        {
                            if let Some(mapped_exports) =
                                self.modules_in_scope.get(imported_module_id)
                            {
                                items = Some(vec![]);

                                for x in &import_decl.specifiers {
                                    match x {
                                        ImportSpecifier::Named(named_specifier) => {
                                            // handle conflict name in top level

                                            let imported_symbol = if let Some(imported) =
                                                &named_specifier.imported
                                                && let ModuleExportName::Ident(imported_ident) =
                                                    imported
                                            {
                                                imported_ident.sym.to_string()
                                            } else {
                                                named_specifier.local.sym.to_string()
                                            };

                                            let mapped_export =
                                                mapped_exports.get(&imported_symbol).unwrap();

                                            if !named_specifier
                                                .local
                                                .sym
                                                .to_string()
                                                .eq(mapped_export)
                                            {
                                                let var_decl_stmt: Stmt =
                                                    quote_ident!(mapped_export.clone())
                                                        .into_var_decl(
                                                            VarDeclKind::Var,
                                                            named_specifier.local.clone().into(),
                                                        )
                                                        .into();
                                                if let Some(a) = items.as_mut() {
                                                    a.push(var_decl_stmt.into());
                                                }
                                            }
                                        }
                                        ImportSpecifier::Default(default_specifier) => {
                                            let mapped_default =
                                                mapped_exports.get("default").unwrap();

                                            self.request_rename((
                                                Id::from(default_specifier.local.clone()),
                                                (
                                                    mapped_default.clone().into(),
                                                    default_specifier.local.span.ctxt,
                                                ),
                                            ));
                                        }
                                        ImportSpecifier::Namespace(namespace) => {
                                            let mapped_namespace = mapped_exports.get("*").unwrap();

                                            if !namespace.local.sym.to_string().eq(mapped_namespace)
                                            {
                                                let var_decl_stmt: Stmt =
                                                    quote_ident!(mapped_namespace.clone())
                                                        .into_var_decl(
                                                            VarDeclKind::Var,
                                                            namespace.local.clone().into(),
                                                        )
                                                        .into();
                                                if let Some(a) = items.as_mut() {
                                                    a.push(var_decl_stmt.into());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
                                            let (local, exported) =
                                                export_named_specifier_to_local_and_exported(named);

                                            if let Some(mapped_export) = export_map.get(&exported) {
                                                let i = items.as_mut().unwrap();

                                                if local.sym.eq(mapped_export) {
                                                    let module_dcl: ModuleDecl = NamedExport {
                                                        span: Default::default(),
                                                        specifiers: vec![ExportNamedSpecifier {
                                                            span: Default::default(),
                                                            orig: ModuleExportName::Ident(
                                                                local.clone(),
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
                                                    let export_decl: ModuleDecl = ExportDecl {
                                                        span: Default::default(),
                                                        decl: Decl::Var(
                                                            declare_var_with_init(
                                                                local.clone(),
                                                                mapped_export,
                                                            )
                                                            .into(),
                                                        ),
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
                    ModuleDecl::ExportAll(_) => {}
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

        let map = self.renames.iter().cloned().collect();
        let mut renamer = IdentRenamer::new(&map);
        n.visit_mut_with(&mut renamer);

        self.resolve_conflicts(n);
    }
}

fn export_named_specifier_to_local_and_exported(named: &ExportNamedSpecifier) -> (Ident, String) {
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
