use std::collections::HashSet;

use swc_ecma_ast::{Decl, ExportDecl, ImportDecl};
use swc_ecma_visit::{VisitMut, VisitWith};

use crate::comments::Comments;
use crate::defined_ident_collector::DefinedIdentCollector;
use crate::statement::{self, ExportSpecifier, ImportSpecifier, StatementId, StatementType};
use crate::tree_shaking_module::{TreeShakingModule, UsedIdent};

/**
 * 针对没有使用到的 export、import 语句进行标记删除
 */
pub struct UnusedStatementMarker<'a, 'b> {
    tree_shaking_module: &'a TreeShakingModule,
    used_export_statement: Vec<(StatementId, HashSet<UsedIdent>)>,
    comments: &'b mut Comments,
}

impl<'a, 'b> UnusedStatementMarker<'a, 'b> {
    pub fn new(tree_shaking_module: &'a TreeShakingModule, comments: &'b mut Comments) -> Self {
        Self {
            tree_shaking_module,
            used_export_statement: tree_shaking_module.get_used_export_statement().into(),
            comments,
        }
    }
}

impl VisitMut for UnusedStatementMarker<'_, '_> {
    fn visit_mut_export_decl(&mut self, export_decl: &mut ExportDecl) {
        // 清理没有用到的 decl
        for (statement_id, ..) in &self.used_export_statement {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            match statement {
                StatementType::Export(export_statement) => match &mut export_decl.decl {
                    Decl::Var(var_decl) => {
                        for decl in &var_decl.decls {
                            if !is_same_decl(export_statement, decl) {
                                self.comments.add_unused_comment(decl.span.lo)
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        // 清理没有用到的 specifier
        for (statement_id, ..) in &self.used_export_statement {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            match statement {
                StatementType::Import(import_statement) => {
                    for specifier in &import_decl.specifiers {
                        match specifier {
                            swc_ecma_ast::ImportSpecifier::Named(named_specifier) => {
                                if !is_same_import_specifier(import_statement, named_specifier) {
                                    self.comments.add_unused_comment(named_specifier.span.lo)
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn is_same_decl(
    export_statement: &statement::ExportStatement,
    decl: &swc_ecma_ast::VarDeclarator,
) -> bool {
    export_statement
        .info
        .specifiers
        .iter()
        .any(|export_specifier| match export_specifier {
            ExportSpecifier::Named { local, .. } => {
                let mut defined_ident_collector = DefinedIdentCollector::new();
                decl.name.visit_with(&mut defined_ident_collector);
                defined_ident_collector.defined_ident.contains(local)
            }
            _ => false,
        })
}

fn is_same_import_specifier(
    import_statement: &crate::statement::ImportStatement,
    named_specifier: &swc_ecma_ast::ImportNamedSpecifier,
) -> bool {
    import_statement
        .info
        .specifiers
        .iter()
        .any(|specifier| match specifier {
            ImportSpecifier::Named { local, .. } => named_specifier.local.to_string() == *local,
            _ => false,
        })
}
