use mako_core::swc_ecma_ast::{
    Decl, ExportDecl, ExportSpecifier, ImportDecl, ImportSpecifier, Module, ModuleExportName,
};
use mako_core::swc_ecma_visit::{Visit, VisitWith};

use crate::module::{ModuleInfo, OptimsType, UselessStmtType};
use crate::plugins::farm_tree_shake::module::TreeShakeModule;
use crate::plugins::farm_tree_shake::statement_graph::analyze_imports_and_exports::{
    analyze_imports_and_exports, StatementInfo,
};
use crate::plugins::farm_tree_shake::statement_graph::defined_idents_collector::DefinedIdentsCollector;
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo as UsedExportSpecInfo, ImportInfo, ImportSpecifierInfo,
};

pub fn mark_useless_stmts(
    tree_shake_module: &mut TreeShakeModule,
    module_info: &mut ModuleInfo,
) -> (Vec<ImportInfo>, Vec<ExportInfo>) {
    // analyze the statement graph start from the used statements
    let used_stmts = tree_shake_module
        .used_statements()
        .into_iter()
        .collect::<Vec<_>>();

    let mut used_import_infos = vec![];
    let mut used_export_from_infos = vec![];
    let swc_module = module_info.ast.as_script().unwrap();

    // remove unused specifiers in export statement and import statement
    for (stmt_id, used_defined_idents) in &used_stmts {
        let module_item = &swc_module.ast.body[*stmt_id];

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

            let mut remover = UselessImportStmtsMarker {
                import_info,
                module_optims: &mut module_info.optims,
            };

            module_item.visit_with(&mut remover);
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
                    let mut remover = UselessExportStmtMarker {
                        export_info,
                        module_optims: &mut module_info.optims,
                    };

                    module_item.visit_with(&mut remover);
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

                        let mut remover = UselessExportStmtMarker {
                            export_info,
                            module_optims: &mut module_info.optims,
                        };

                        module_item.visit_with(&mut remover);
                    }
                }
            } else {
                // export { a ,b } or export default a;
                let mut remover = UselessExportStmtMarker {
                    export_info,
                    module_optims: &mut module_info.optims,
                };

                module_item.visit_with(&mut remover);
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
    for (index, _) in swc_module.ast.body.iter().enumerate() {
        if !used_stmts_indexes.contains(&&index) {
            stmts_to_remove.push(index);
        }
    }

    // remove from the end to the start
    stmts_to_remove.reverse();

    for stmt in stmts_to_remove {
        module_info
            .optims
            .push(OptimsType::UselessStmt(stmt, UselessStmtType::Stmt));
    }

    (used_import_infos, used_export_from_infos)
}

pub fn remove_useless_stmts(module_info: &ModuleInfo, swc_module: &mut Module) {
    module_info.optims.iter().for_each(|optim| match optim {
        OptimsType::UselessStmt(stmt_id, useless_stmt_type) => match useless_stmt_type {
            UselessStmtType::ImportSpecifier(sp_index) => {
                swc_module.body[*stmt_id]
                    .as_mut_module_decl()
                    .unwrap()
                    .as_mut_import()
                    .unwrap()
                    .specifiers
                    .remove(*sp_index);
            }
            UselessStmtType::ExportDecl(decl_index) => {
                swc_module.body[*stmt_id]
                    .as_mut_module_decl()
                    .unwrap()
                    .as_mut_export_decl()
                    .unwrap()
                    .decl
                    .as_mut_var()
                    .unwrap()
                    .decls
                    .remove(*decl_index);
            }
            UselessStmtType::ExportSpecifier(sp_index) => {
                swc_module.body[*stmt_id]
                    .as_mut_module_decl()
                    .unwrap()
                    .as_mut_export_named()
                    .unwrap()
                    .specifiers
                    .remove(*sp_index);
            }
            UselessStmtType::Stmt => {
                swc_module.body.remove(*stmt_id);
            }
        },
        _ => unreachable!(),
    });
}

pub struct UselessImportStmtsMarker<'a> {
    import_info: ImportInfo,
    module_optims: &'a mut Vec<OptimsType>,
}

impl<'a> Visit for UselessImportStmtsMarker<'a> {
    // 1. import { a } from 'x';
    // 2. import a from 'x';
    // 3. import * as a from 'x';
    // if specifier is not used and x has sideEffect, convert to import 'x';

    fn visit_import_decl(&mut self, import_decl: &ImportDecl) {
        let mut specifiers_to_remove = vec![];

        for (index, specifier) in import_decl.specifiers.iter().enumerate() {
            if !self.import_info.specifiers.iter().any(|specifier_info| {
                match (specifier_info, specifier) {
                    (
                        ImportSpecifierInfo::Named { local, .. },
                        ImportSpecifier::Named(named_specifier),
                    ) => named_specifier.local.to_string() == *local,
                    (
                        ImportSpecifierInfo::Default(str),
                        ImportSpecifier::Default(default_specifier),
                    ) => default_specifier.local.to_string() == *str,
                    (
                        ImportSpecifierInfo::Namespace(str),
                        ImportSpecifier::Namespace(namespace_specifier),
                    ) => namespace_specifier.local.to_string() == *str,
                    _ => false,
                }
            }) {
                specifiers_to_remove.push(index);
            }
        }

        specifiers_to_remove.reverse();

        for index in specifiers_to_remove {
            self.module_optims.push(OptimsType::UselessStmt(
                self.import_info.stmt_id,
                UselessStmtType::ImportSpecifier(index),
            ));
        }
    }
}

pub struct UselessExportStmtMarker<'a> {
    export_info: ExportInfo,
    module_optims: &'a mut Vec<OptimsType>,
}

impl<'a> Visit for UselessExportStmtMarker<'a> {
    fn visit_export_decl(&mut self, export_decl: &ExportDecl) {
        if let Decl::Var(var_decl) = &export_decl.decl {
            let mut decls_to_remove = vec![];

            for (index, decl) in var_decl.decls.iter().enumerate() {
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
                self.module_optims.push(OptimsType::UselessStmt(
                    self.export_info.stmt_id,
                    UselessStmtType::ExportDecl(index),
                ));
            }
        }
    }

    fn visit_export_specifiers(&mut self, specifiers: &[ExportSpecifier]) {
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
            self.module_optims.push(OptimsType::UselessStmt(
                self.export_info.stmt_id,
                UselessStmtType::ExportSpecifier(index),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::tests::{TestUtils, TestUtilsOpts};

    #[test]
    fn remove_unused_default_import() {
        assert_eq!(
            remove_unused_specifier(
                r#"import unused, {used} from "m""#,
                ImportSpecifierInfo::Named {
                    local: "used#0".to_string(),
                    imported: None
                }
            ),
            r#"import { used } from "m";"#
        );
    }

    #[test]
    fn remove_unused_named_import() {
        assert_eq!(
            remove_unused_specifier(
                r#"import used, {unused} from "m""#,
                ImportSpecifierInfo::Default("used#0".to_string())
            ),
            r#"import used from "m";"#
        );
    }

    #[test]
    fn remove_unused_namespaced_import() {
        assert_eq!(
            remove_unused_specifier(
                r#"import used, * as unused from "m""#,
                ImportSpecifierInfo::Default("used#0".to_string()),
            ),
            r#"import used from "m";"#
        );
    }

    fn remove_unused_specifier(code: &str, used_import_specifier: ImportSpecifierInfo) -> String {
        let mut tu = TestUtils::new(TestUtilsOpts {
            file: Some("test.js".to_string()),
            content: Some(code.to_string()),
        });
        let mut module_info = ModuleInfo::default();

        tu.ast.js().ast.visit_with(&mut UselessImportStmtsMarker {
            import_info: ImportInfo {
                source: "m".to_string(),
                specifiers: vec![used_import_specifier],
                stmt_id: 0,
            },
            module_optims: &mut module_info.optims,
        });

        remove_useless_stmts(&module_info, &mut tu.ast.js_mut().ast);

        tu.js_ast_to_code()
    }
}
