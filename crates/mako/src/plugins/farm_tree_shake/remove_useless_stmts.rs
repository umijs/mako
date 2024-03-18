use mako_core::swc_ecma_ast::{
    Decl, ExportDecl, ExportSpecifier, ImportDecl, ImportSpecifier, Module as SwcModule,
    ModuleExportName,
};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith, VisitWith};

use crate::plugins::farm_tree_shake::module::TreeShakeModule;
use crate::plugins::farm_tree_shake::statement_graph::analyze_imports_and_exports::{
    analyze_imports_and_exports, StatementInfo,
};
use crate::plugins::farm_tree_shake::statement_graph::defined_idents_collector::DefinedIdentsCollector;
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo as UsedExportSpecInfo, ImportInfo,
};

pub fn remove_useless_stmts(
    tree_shake_module: &mut TreeShakeModule,
    swc_module: &SwcModule,
) -> (Vec<ImportInfo>, Vec<ExportInfo>, SwcModule) {
    // analyze the statement graph start from the used statements
    let mut used_stmts = tree_shake_module
        .used_statements()
        .into_iter()
        .collect::<Vec<_>>();
    // sort used_stmts
    used_stmts.sort_by_key(|a| a.0);

    let mut used_import_infos = vec![];
    let mut used_export_from_infos = vec![];

    // remove unused specifiers in export statement and import statement
    let mut swc_module = swc_module.clone();
    for (stmt_id, used_defined_idents) in &used_stmts {
        let module_item = &mut swc_module.body[*stmt_id];

        let StatementInfo {
            import_info,
            export_info,
            ..
        } = analyze_imports_and_exports(
            stmt_id,
            module_item,
            Some(used_defined_idents.clone()),
            tree_shake_module.unresolved_ctxt,
        );

        if let Some(import_info) = import_info {
            used_import_infos.push(import_info.clone());

            let mut remover = UselessImportStmtsRemover { import_info };

            module_item.visit_mut_with(&mut remover);
        }

        if let Some(mut export_info) = export_info {
            // ignore export {}
            if export_info.specifiers.is_empty() && export_info.source.is_none() {
                continue;
            }

            if export_info.source.is_some() {
                // export {} from "x"
                if export_info.specifiers.is_empty() {
                    used_export_from_infos.push(export_info.clone());
                    let mut remover = UselessExportStmtRemover { export_info };

                    module_item.visit_mut_with(&mut remover);
                } else {
                    // export * from  "x"
                    if matches!(export_info.specifiers[0], UsedExportSpecInfo::All(_)) {
                        export_info.specifiers[0] = UsedExportSpecInfo::All(
                            used_defined_idents.clone().into_iter().collect(),
                        );

                        used_export_from_infos.push(export_info.clone());
                    } else {
                        // export {a,b } from "x"
                        used_export_from_infos.push(export_info.clone());

                        let mut remover = UselessExportStmtRemover { export_info };

                        module_item.visit_mut_with(&mut remover);
                    }
                }
            } else {
                // export { a ,b } or export default a;
                let mut remover = UselessExportStmtRemover { export_info };

                module_item.visit_mut_with(&mut remover);
            }
        }
    }

    let mut stmts_to_remove = vec![];
    // TODO recognize the self-executed statements and preserve all the related statements

    let used_stmts_indexes = used_stmts
        .iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    // remove the unused statements from the module
    for (index, _) in swc_module.body.iter().enumerate() {
        if !used_stmts_indexes.contains(&&index) {
            stmts_to_remove.push(index);
        }
    }

    // remove from the end to the start
    stmts_to_remove.reverse();

    for stmt in stmts_to_remove {
        swc_module.body.remove(stmt);
    }

    (used_import_infos, used_export_from_infos, swc_module)
}

pub struct UselessImportStmtsRemover {
    import_info: ImportInfo,
}

impl VisitMut for UselessImportStmtsRemover {
    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        let mut specifiers_to_remove = vec![];

        for (index, specifier) in import_decl.specifiers.iter().enumerate() {
            if let ImportSpecifier::Named(named_specifier) = specifier {
                if !self.
                    import_info.specifiers
          .iter()
          .any(|specifier| match specifier {
            crate::plugins::farm_tree_shake::statement_graph::ImportSpecifierInfo::Named { local, .. } => named_specifier.local.to_string() == *local,
            _ => false,
          })
        {
          specifiers_to_remove.push(index);
        }
            }
        }

        specifiers_to_remove.reverse();

        for index in specifiers_to_remove {
            import_decl.specifiers.remove(index);
        }
    }
}

pub struct UselessExportStmtRemover {
    export_info: ExportInfo,
}

impl VisitMut for UselessExportStmtRemover {
    fn visit_mut_export_decl(&mut self, export_decl: &mut ExportDecl) {
        if let Decl::Var(var_decl) = &mut export_decl.decl {
            let mut decls_to_remove = vec![];

            for (index, decl) in var_decl.decls.iter_mut().enumerate() {
                if !self.export_info.specifiers.iter().any(
                    |export_specifier| match export_specifier {
                        UsedExportSpecInfo::Named { local, .. } => {
                            let mut defined_idents_collector = DefinedIdentsCollector::new();
                            decl.name.visit_with(&mut defined_idents_collector);

                            defined_idents_collector.defined_idents.contains(local)
                        }
                        _ => false,
                    },
                ) {
                    decls_to_remove.push(index);
                }
            }

            decls_to_remove.reverse();

            for index in decls_to_remove {
                var_decl.decls.remove(index);
            }
        }
    }

    fn visit_mut_export_specifiers(&mut self, specifiers: &mut Vec<ExportSpecifier>) {
        let mut specifiers_to_remove = vec![];

        for (index, specifier) in specifiers.iter().enumerate() {
            if !self
                .export_info
                .specifiers
                .iter()
                .any(
                    |used_export_specifier| match (used_export_specifier, specifier) {
                        (
                            UsedExportSpecInfo::Named { local, .. },
                            ExportSpecifier::Named(named_specifier),
                        ) => match &named_specifier.orig {
                            ModuleExportName::Ident(ident) => ident.to_string() == *local,
                            _ => false,
                        },

                        (
                            UsedExportSpecInfo::Namespace(used_namespace),
                            ExportSpecifier::Namespace(namespace),
                        ) => match &namespace.name {
                            ModuleExportName::Ident(ident) => ident.to_string() == *used_namespace,
                            _ => false,
                        },

                        (_, _) => false,
                    },
                )
            {
                specifiers_to_remove.push(index);
            }
        }

        specifiers_to_remove.reverse();

        for index in specifiers_to_remove {
            specifiers.remove(index);
        }
    }
}
