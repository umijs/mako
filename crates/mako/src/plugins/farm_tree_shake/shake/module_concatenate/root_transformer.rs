use std::sync::Arc;

use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Spanned, DUMMY_SP};
use swc_core::ecma::ast::{ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, ExprFactory};
use swc_core::ecma::visit::VisitMut;

use super::utils::uniq_module_prefix;
use crate::compiler::Context;
use crate::module::{relative_to_root, ModuleId};
use crate::module_graph::ModuleGraph;

pub(in crate::plugins) struct RootTransformer<'a> {
    pub module_graph: &'a ModuleGraph,
    pub current_module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
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
                let mut items: Vec<ModuleItem> = vec![];

                match module_dc {
                    ModuleDecl::Import(import_decl) => {
                        let source = import_decl.src.value.to_string();

                        let imported_module = self
                            .module_graph
                            .get_dependency_module_by_source(self.current_module_id, &source)
                            .unwrap();

                        for x in &import_decl.specifiers {
                            match x {
                                ImportSpecifier::Named(_) => {
                                    // handle conflict name in top level
                                }
                                ImportSpecifier::Default(default_name) => {
                                    let decl = quote_ident!(format!(
                                        "{}_0",
                                        uniq_module_prefix(imported_module, self.context)
                                    ))
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        default_name.local.clone().into(),
                                    );

                                    let stmt: Stmt = decl.into();

                                    items.push(stmt.into());
                                }
                                ImportSpecifier::Namespace(_) => {}
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
    }
}
