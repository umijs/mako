use std::collections::HashSet;

use swc_ecma_ast::{
    ClassDecl, Decl, ExportDecl, FnDecl, Ident, ImportDecl, ModuleExportName, VarDeclarator,
};
use swc_ecma_visit::{VisitMut, VisitWith};
use tracing::debug;

use crate::comments::Comments;
use crate::defined_ident_collector::DefinedIdentCollector;
use crate::statement::{self, ExportSpecifier, ImportSpecifier, StatementId, StatementType};
use crate::tree_shaking_module::{TreeShakingModule, UsedIdent};
/**
 * 针对没有使用到的 export、import 语句进行标记
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
    // 清理 export { } 这里面的变量
    fn visit_mut_export_specifiers(&mut self, specifiers: &mut Vec<swc_ecma_ast::ExportSpecifier>) {
        for (_, specifier) in specifiers.iter().enumerate() {
            if let swc_ecma_ast::ExportSpecifier::Named(named_specifier) = specifier {
                let is_specifier_used = self.is_export_specifier_used(named_specifier);

                if !is_specifier_used {
                    debug!("add unused comment to {:?}", &named_specifier);
                    self.comments.add_unused_comment(named_specifier.span.lo);
                }
            }
        }
    }

    fn visit_mut_export_decl(&mut self, export_decl: &mut ExportDecl) {
        match &mut export_decl.decl {
            Decl::Var(var_decl) => {
                for decl in &var_decl.decls {
                    let is_decl_used = self.is_var_decl_used(decl);

                    if !is_decl_used {
                        debug!("add unused comment to {:?}", &decl.name);
                        self.comments.add_unused_comment(decl.span.lo);
                    }
                }
            }
            Decl::Class(class_decl) => {
                let is_decl_used = self.is_class_decl_used(class_decl);

                if !is_decl_used {
                    debug!("add unused comment to {:?}", &class_decl.ident);
                    self.comments.add_unused_comment(class_decl.ident.span.lo);
                }
            }
            Decl::Fn(fn_decl) => {
                let is_decl_used = self.is_fn_decl_used(fn_decl);

                if !is_decl_used {
                    debug!("add unused comment to {:?}", &fn_decl.ident);
                    self.comments.add_unused_comment(fn_decl.ident.span.lo)
                }
            }
            _ => (),
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

impl<'a, 'b> UnusedStatementMarker<'a, 'b> {
    fn is_export_specifier_used(
        &mut self,
        named_specifier: &swc_ecma_ast::ExportNamedSpecifier,
    ) -> bool {
        self.used_export_statement.iter().any(|(statement_id, ..)| {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            if let StatementType::Export(export_statement) = statement {
                let local = match &named_specifier.orig {
                    ModuleExportName::Ident(i) => i.clone(),
                    ModuleExportName::Str(_) => {
                        unreachable!("str as ident is not supported")
                    }
                };
                is_same_ident(export_statement, &local)
            } else {
                false
            }
        })
    }
}

impl<'a, 'b> UnusedStatementMarker<'a, 'b> {
    fn is_var_decl_used(&mut self, decl: &VarDeclarator) -> bool {
        let is_decl_used = &self.used_export_statement.iter().any(|(statement_id, ..)| {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            if let StatementType::Export(export_statement) = statement {
                is_same_decl(export_statement, decl)
            } else {
                false
            }
        });
        *is_decl_used
    }

    fn is_fn_decl_used(&mut self, decl: &FnDecl) -> bool {
        let is_decl_used = &self.used_export_statement.iter().any(|(statement_id, ..)| {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            if let StatementType::Export(export_statement) = statement {
                is_same_ident(export_statement, &decl.ident)
            } else {
                false
            }
        });
        *is_decl_used
    }

    fn is_class_decl_used(&mut self, decl: &ClassDecl) -> bool {
        let is_decl_used = &self.used_export_statement.iter().any(|(statement_id, ..)| {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            if let StatementType::Export(export_statement) = statement {
                is_same_ident(export_statement, &decl.ident)
            } else {
                false
            }
        });
        *is_decl_used
    }
}

fn is_same_decl(
    export_statement: &statement::ExportStatement,
    decl: &dyn VisitWith<DefinedIdentCollector>,
) -> bool {
    let mut defined_ident_collector = DefinedIdentCollector::new();
    decl.visit_with(&mut defined_ident_collector);

    export_statement
        .info
        .specifiers
        .iter()
        .any(|export_specifier| match export_specifier {
            ExportSpecifier::Named { local, exported } => {
                if let Some(exported) = exported {
                    defined_ident_collector.defined_ident.contains(exported)
                } else {
                    defined_ident_collector.defined_ident.contains(local)
                }
            }
            _ => false,
        })
}

fn is_same_ident(export_statement: &statement::ExportStatement, ident: &Ident) -> bool {
    export_statement
        .info
        .specifiers
        .iter()
        .any(|export_specifier| match export_specifier {
            ExportSpecifier::Named { local, .. } => ident.to_string() == *local,
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
