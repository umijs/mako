use std::collections::HashSet;

use swc_core::common::util::take::Take;
use swc_core::common::SyntaxContext;
use swc_core::ecma::ast::{
    Decl, ExportDecl, ExportSpecifier, Id, ImportDecl, ImportSpecifier, Module as SwcModule,
    Module, ModuleExportName,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith, VisitWith};

use super::collect_explicit_prop::IdExplicitPropAccessCollector;
use crate::plugins::tree_shaking::module::TreeShakeModule;
use crate::plugins::tree_shaking::statement_graph::analyze_imports_and_exports::{
    analyze_imports_and_exports, StatementInfo,
};
use crate::plugins::tree_shaking::statement_graph::defined_idents_collector::DefinedIdentsCollector;
use crate::plugins::tree_shaking::statement_graph::{
    ExportInfo, ExportSpecifierInfo as UsedExportSpecInfo, ImportInfo, ImportSpecifierInfo,
};

pub fn remove_useless_stmts(
    tree_shake_module: &mut TreeShakeModule,
    swc_module: &mut SwcModule,
) -> (Vec<ImportInfo>, Vec<ExportInfo>) {
    // analyze the statement graph start from the used statements
    let used_stmts = tree_shake_module
        .used_statements()
        .into_iter()
        .collect::<Vec<_>>();

    let mut used_import_infos = vec![];
    let mut used_export_from_infos = vec![];

    // remove unused specifiers in export statement and import statement
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

    optimize_import_namespace(&mut used_import_infos, swc_module);

    (used_import_infos, used_export_from_infos)
}

pub struct UselessImportStmtsRemover {
    import_info: ImportInfo,
}

impl VisitMut for UselessImportStmtsRemover {
    // 1. import { a } from 'x';
    // 2. import a from 'x';
    // 3. import * as a from 'x';
    // if specifier is not used and x has sideEffect, convert to import 'x';

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
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

fn optimize_import_namespace(import_infos: &mut [ImportInfo], module: &mut Module) {
    let namespaces = import_infos
        .iter()
        .filter_map(|import_info| {
            let ns = import_info
                .specifiers
                .iter()
                .filter_map(|sp| match sp {
                    ImportSpecifierInfo::Namespace(ns) => Some(ns.clone()),
                    _ => None,
                })
                .collect::<Vec<String>>();
            if ns.is_empty() {
                None
            } else {
                Some(ns)
            }
        })
        .flatten()
        .collect::<Vec<String>>();

    let ids = namespaces
        .iter()
        .map(|ns| {
            let (sym, ctxt) = ns.rsplit_once('#').unwrap();
            (sym.into(), SyntaxContext::from_u32(ctxt.parse().unwrap()))
        })
        .collect::<HashSet<Id>>();

    let mut v = IdExplicitPropAccessCollector::new(ids);
    module.visit_with(&mut v);

    let explicit_prop_accessed_ids = v.explicit_accessed_props();

    import_infos.iter_mut().for_each(|ii| {
        ii.specifiers = ii
            .specifiers
            .take()
            .into_iter()
            .flat_map(|specifier_info| {
                if let ImportSpecifierInfo::Namespace(ref ns) = specifier_info {
                    if let Some(visited_fields) = explicit_prop_accessed_ids.get(ns) {
                        return visited_fields
                            .iter()
                            .map(|v| {
                                let imported_name = format!("{v}#0");
                                ImportSpecifierInfo::Named {
                                    imported: Some(imported_name.clone()),
                                    local: imported_name,
                                }
                            })
                            .collect::<Vec<_>>();
                    }
                }
                vec![specifier_info]
            })
            .collect::<Vec<_>>();
    })
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

        tu.ast
            .js_mut()
            .ast
            .visit_mut_with(&mut UselessImportStmtsRemover {
                import_info: ImportInfo {
                    source: "m".to_string(),
                    specifiers: vec![used_import_specifier],
                    stmt_id: 0,
                },
            });

        tu.js_ast_to_code()
    }
}
