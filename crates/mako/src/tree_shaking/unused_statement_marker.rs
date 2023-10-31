use std::collections::HashSet;

use mako_core::swc_ecma_ast;
use mako_core::swc_ecma_ast::{Decl, ExportDecl, ImportDecl, ModuleExportName, VarDeclarator};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith, VisitWith};
use mako_core::tracing::debug;

use crate::comments::Comments;
use crate::tree_shaking::defined_ident_collector::DefinedIdentCollector;
use crate::tree_shaking::statement::{
    self, ExportSpecifier, ImportSpecifier, StatementId, StatementType,
};
use crate::tree_shaking::tree_shaking_analyze::strip_context;
use crate::tree_shaking::tree_shaking_module::{
    should_skip, TreeShakingModule, UsedIdent, UsedIdentHashMap,
};
/**
 * 针对没有使用到的 export、import 语句进行标记
 */
pub struct UnusedStatementMarker<'a, 'b> {
    tree_shaking_module: &'a TreeShakingModule,
    used_export_statement: Vec<(StatementId, HashSet<UsedIdent>)>,
    comments: &'b mut Comments,
}

impl<'a, 'b> UnusedStatementMarker<'a, 'b> {
    pub fn new(
        tree_shaking_module: &'a TreeShakingModule,
        used_export_statement: UsedIdentHashMap,
        comments: &'b mut Comments,
    ) -> Self {
        let used_export_statement = used_export_statement.into();
        Self {
            tree_shaking_module,
            used_export_statement,
            comments,
        }
    }
}

impl VisitMut for UnusedStatementMarker<'_, '_> {
    // 清理空模块，打上注释
    fn visit_mut_module(&mut self, module: &mut swc_ecma_ast::Module) {
        if should_skip(&self.tree_shaking_module.id.id) {
            debug!("skip module {:?}", &self.tree_shaking_module.id);
            return;
        }
        if self.tree_shaking_module.used_exports.is_empty() {
            self.comments.add_unused_module_comment(module.span.lo);
        } else {
            module.visit_mut_children_with(self);
        }
    }

    // 清理 export { } 这里面的变量
    fn visit_mut_export_specifiers(&mut self, specifiers: &mut Vec<swc_ecma_ast::ExportSpecifier>) {
        for (_, specifier) in specifiers.iter().enumerate() {
            match specifier {
                swc_ecma_ast::ExportSpecifier::Namespace(_) => {}
                swc_ecma_ast::ExportSpecifier::Default(_) => {}
                swc_ecma_ast::ExportSpecifier::Named(named_specifier) => {
                    let is_specifier_used = self.is_named_export_specifier_used(named_specifier);

                    if !is_specifier_used {
                        debug!("add unused comment to {:?}", &named_specifier);
                        self.comments.add_unused_comment(named_specifier.span.lo);
                    }
                }
            }
        }
    }

    fn visit_mut_export_default_expr(
        &mut self,
        default_expr: &mut swc_ecma_ast::ExportDefaultExpr,
    ) {
        if !self.tree_shaking_module.used_exports.contains(&"default") {
            debug!("add unused comment to default fn");
            self.comments.add_unused_comment(default_expr.span.lo)
        }
    }

    fn visit_mut_export_default_decl(
        &mut self,
        default_decl: &mut swc_ecma_ast::ExportDefaultDecl,
    ) {
        if !self.tree_shaking_module.used_exports.contains(&"default") {
            debug!("add unused comment to default fn");
            self.comments.add_unused_comment(default_decl.span.lo)
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
                let is_decl_used = self.is_fn_or_class_decl_ident_used(&class_decl.ident);

                if !is_decl_used {
                    debug!("add unused comment to {:?}", &class_decl.ident);
                    self.comments.add_unused_comment(class_decl.ident.span.lo);
                }
            }
            Decl::Fn(fn_decl) => {
                let is_decl_used = self.is_fn_or_class_decl_ident_used(&fn_decl.ident);

                if !is_decl_used {
                    debug!("add unused comment to {:?}", &fn_decl.ident);
                    self.comments.add_unused_comment(fn_decl.ident.span.lo)
                }
            }
            _ => (),
        }
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        if should_skip(&import_decl.src.value) {
            return;
        }
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
    fn is_named_export_specifier_used(
        &mut self,
        named_specifier: &swc_ecma_ast::ExportNamedSpecifier,
    ) -> bool {
        self.used_export_statement
            .iter()
            .any(|(statement_id, used_idents)| {
                let statement = self.tree_shaking_module.get_statement(statement_id);
                if let StatementType::Export(export_statement) = statement {
                    let local = match &named_specifier.orig {
                        ModuleExportName::Ident(i) => i.clone(),
                        ModuleExportName::Str(_) => {
                            unreachable!("str as ident is not supported")
                        }
                    };
                    if !is_same_export_ident(export_statement, &local) {
                        return false;
                    }
                    // 可能在 export {} 中使用部分变量，需要单独把他们识别出来
                    let exported = if let Some(exported) = &named_specifier.exported {
                        exported
                    } else {
                        &named_specifier.orig
                    };
                    let local = match exported {
                        ModuleExportName::Ident(i) => i.clone(),
                        ModuleExportName::Str(_) => {
                            unreachable!("str as ident is not supported")
                        }
                    };

                    used_idents.contains(&UsedIdent::SwcIdent(local.to_string()))
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
                is_same_export_decl(export_statement, decl)
            } else {
                false
            }
        });
        *is_decl_used
    }

    fn is_fn_or_class_decl_ident_used(&mut self, ident: &dyn ToString) -> bool {
        let is_decl_used = &self.used_export_statement.iter().any(|(statement_id, ..)| {
            let statement = self.tree_shaking_module.get_statement(statement_id);
            if let StatementType::Export(export_statement) = statement {
                is_same_export_ident(export_statement, ident)
            } else {
                false
            }
        });
        *is_decl_used
    }
}

fn is_same_export_decl(
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

fn is_same_export_ident(
    export_statement: &statement::ExportStatement,
    ident: &dyn ToString,
) -> bool {
    export_statement
        .info
        .specifiers
        .iter()
        .any(|export_specifier| match export_specifier {
            ExportSpecifier::Named { local, .. } => ident.to_string() == *local,
            ExportSpecifier::Default => strip_context(&ident.to_string()) == "default",
            _ => false,
        })
}

fn is_same_import_specifier(
    import_statement: &crate::tree_shaking::statement::ImportStatement,
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
