use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, DUMMY_SP};
use swc_core::ecma::ast::{Id, ImportSpecifier, Module, ModuleDecl, ModuleExportName, ModuleItem};
use swc_core::ecma::utils::IdentRenamer;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::{relative_to_root, ModuleId};
use crate::module_graph::ModuleGraph;

pub(super) struct RootTransformer<'a> {
    pub module_graph: &'a ModuleGraph,
    pub current_module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
    pub modules_in_scope: &'a HashMap<ModuleId, HashMap<String, String>>,
    pub top_level_vars: &'a HashSet<String>,
    pub top_level_mark: Mark,
    pub import_source_to_module_id: &'a HashMap<String, ModuleId>,
    pub renames: Vec<(Id, Id)>,
}

impl RootTransformer<'_> {
    fn request_rename(&mut self, req: (Id, Id)) {
        self.renames.push(req);
    }
}

impl<'a> VisitMut for RootTransformer<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let mut replaces = vec![];

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

        for (index, module_item) in n.body.iter().enumerate().rev() {
            if let Some(module_dc) = module_item.as_module_decl() {
                let items: Vec<ModuleItem> = vec![];

                match module_dc {
                    ModuleDecl::Import(import_decl) => {
                        let source = import_decl.src.value.to_string();

                        if let Some(imported_module_id) =
                            self.import_source_to_module_id.get(&source)
                        {
                            if let Some(mapped_exports) =
                                self.modules_in_scope.get(imported_module_id)
                            {
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

                                            self.request_rename((
                                                Id::from(named_specifier.local.clone()),
                                                (
                                                    mapped_export.clone().into(),
                                                    named_specifier.local.span.ctxt,
                                                ),
                                            ));
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
                                        ImportSpecifier::Namespace(_) => {}
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportDecl(_) => {}
                    ModuleDecl::ExportNamed(_) => {}
                    ModuleDecl::ExportDefaultDecl(_) => {}
                    ModuleDecl::ExportDefaultExpr(_) => {}
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }

                replaces.push((index, items));
            }
        }

        for (i, items) in replaces {
            n.body.splice(i..i + 1, items);
        }

        let map = self.renames.iter().cloned().collect();

        let mut renamer = IdentRenamer::new(&map);

        n.visit_mut_with(&mut renamer);
    }
}
