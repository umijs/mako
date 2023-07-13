use std::collections::HashSet;

use swc_ecma_ast::{Decl, ExportDecl, ImportDecl};
use swc_ecma_visit::{VisitMut, VisitWith};
use tracing::debug;

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
        if let Decl::Var(var_decl) = &mut export_decl.decl {
            for decl in &var_decl.decls {
                let is_decl_used = &self.used_export_statement.iter().any(|(statement_id, ..)| {
                    let statement = self.tree_shaking_module.get_statement(statement_id);
                    if let StatementType::Export(export_statement) = statement {
                        is_same_decl(export_statement, decl)
                    } else {
                        false
                    }
                });

                if !is_decl_used {
                    debug!("add unused comment to {:?}", &decl);
                    self.comments.add_unused_comment(decl.span.lo)
                }
            }
        }
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        // 清理没有用到的 specifier
        for specifier in &import_decl.specifiers {
            if let swc_ecma_ast::ImportSpecifier::Named(named_specifier) = specifier {
                let is_specifier_used =
                    &self.used_export_statement.iter().any(|(statement_id, ..)| {
                        let statement = self.tree_shaking_module.get_statement(statement_id);
                        if let StatementType::Import(import_statement) = statement {
                            is_same_import_specifier(import_statement, named_specifier)
                        } else {
                            false
                        }
                    });

                if !is_specifier_used {
                    debug!("add unused comment to {:?}", &named_specifier);
                    self.comments.add_unused_comment(named_specifier.span.lo)
                }
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
