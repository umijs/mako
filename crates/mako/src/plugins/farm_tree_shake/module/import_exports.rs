use crate::plugins::farm_tree_shake::module::{is_ident_sym_equal, TreeShakeModule};
use crate::plugins::farm_tree_shake::shake::{
    strip_context, ReExportSource2, ReExportType, ReExportType2,
};
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfoMatch, ExportSpecifierInfo, ImportSpecifierInfo, Statement,
};

impl TreeShakeModule {
    pub fn find_export_stmt_info(&self, ident: &String) -> Option<&Statement> {
        // find the exported of the ident
        for stmt in self.stmt_graph.stmts() {
            println!("\tthe stake: {:?}", stmt.defined_idents);
            println!("\tthe needle {}", ident);
            if let Some(export_info) = &stmt.export_info {
                // TODO only one Ambiguous can be treated as matched
                if export_info.matches_ident(ident) == ExportInfoMatch::Matched {
                    return Some(stmt);
                }
            } else if stmt.import_info.is_some()
                && stmt
                    .defined_idents
                    .iter()
                    .any(|id_with_ctxt| is_ident_sym_equal(id_with_ctxt, ident))
            {
                return Some(stmt);
            }
        }

        None
    }

    pub fn find_export_define_wip(&self, ident: &String) -> Option<ReExportSource2> {
        let mut local_ident = None;

        for stmt in self.stmt_graph.stmts() {
            println!("\tthe stake: {:?}", stmt.defined_idents);
            println!("\tthe needle {}", ident);
            if let Some(export_info) = &stmt.export_info {
                println!("export_info {:?}", export_info);

                if let Some(export_specifier) = export_info.find_define_specifier(ident) {
                    if let Some(source) = &export_info.source {
                        match export_specifier {
                            ExportSpecifierInfo::All(all_exports) => {
                                if all_exports.iter().any(|i| is_ident_sym_equal(i, ident)) {
                                    return Some(ReExportSource2 {
                                        re_export_type: ReExportType2::Named(strip_context(ident)),
                                        source: source.clone(),
                                    });
                                }
                            }
                            ExportSpecifierInfo::Named { exported, local } => {
                                println!("try to match {} with {:?}", ident, local);

                                let stripped_local = strip_context(local);

                                if let Some(exported_name) = exported {
                                    if is_ident_sym_equal(exported_name, ident) {
                                        return Some(ReExportSource2 {
                                            re_export_type: if stripped_local == "default" {
                                                ReExportType2::Default
                                            } else {
                                                ReExportType2::Named(stripped_local.clone())
                                            },
                                            source: source.clone(),
                                        });
                                    }
                                } else if is_ident_sym_equal(ident, local) {
                                    return Some(ReExportSource2 {
                                        re_export_type: if stripped_local == "default" {
                                            ReExportType2::Default
                                        } else {
                                            ReExportType2::Named(stripped_local.clone())
                                        },
                                        source: source.clone(),
                                    });
                                }
                            }
                            ExportSpecifierInfo::Default(_) => {
                                // Never when export with source
                                return None;
                            }
                            ExportSpecifierInfo::Namespace(_) => {
                                return None;
                            }
                            ExportSpecifierInfo::Ambiguous(_) => {}
                        }
                    } else {
                        match export_specifier {
                            ExportSpecifierInfo::All(_) => {}
                            ExportSpecifierInfo::Named { exported, local } => {
                                println!("try to match {} with {:?}", ident, local);

                                if let Some(exporte_name) = exported {
                                    if is_ident_sym_equal(exporte_name, ident) {
                                        local_ident = Some(local.clone());
                                        break;
                                    }
                                } else if is_ident_sym_equal(ident, local) {
                                    local_ident = Some(local.clone());
                                    break;
                                }
                            }
                            ExportSpecifierInfo::Default(export_default_ident) => {
                                if let Some(default_ident) = export_default_ident
                                    && is_ident_sym_equal(default_ident, ident)
                                {
                                    local_ident = Some(default_ident.clone());
                                }
                            }
                            ExportSpecifierInfo::Namespace(_) => return None,
                            ExportSpecifierInfo::Ambiguous(_) => {
                                // TODO
                                // Ambiguous usually means mixed with cjs, currently cjs
                                // always has side effects
                            }
                        }
                    }
                }
            }
        }

        if let Some(local) = &local_ident {
            for stmt in self.stmt_graph.stmts() {
                if let Some(import_info) = &stmt.import_info {
                    println!("\tthe import stake: {:?}", stmt.defined_idents);
                    println!("\tthe import stmt {:?}", import_info);

                    if let Some(import_specifier) = import_info.find_define_specifier(&local) {
                        match import_specifier {
                            ImportSpecifierInfo::Namespace(name) => {
                                // cant re-export for namespace import
                                return None;
                            }
                            ImportSpecifierInfo::Named {
                                imported,
                                local: imported_local,
                            } => {
                                if is_ident_sym_equal(local, imported_local) {
                                    let next_name = if let Some(imported) = imported {
                                        imported.clone()
                                    } else {
                                        local.clone()
                                    };

                                    return Some(ReExportSource2 {
                                        re_export_type: ReExportType2::Named(strip_context(
                                            &next_name,
                                        )),
                                        source: import_info.source.clone(),
                                    });
                                }
                            }
                            ImportSpecifierInfo::Default(name) => {
                                if local == name {
                                    return Some(ReExportSource2 {
                                        re_export_type: ReExportType2::Default,
                                        source: import_info.source.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            None
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use std::sync::Arc;

    use swc_core::common::GLOBALS;

    use super::TreeShakeModule;
    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::module::{Module, ModuleAst, ModuleInfo};
    use crate::plugins::farm_tree_shake::shake::ReExportSource2;

    impl ReExportSource2 {
        pub fn describe(&self) -> String {
            format!("ReExport from {} by {:?}", self.source, self.re_export_type)
        }
    }

    #[test]
    fn test_find_import_default_export_named() {
        let tsm = tsm_with_code(r#" import a from "./a.js"; export {a}; "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            "ReExport from ./a.js by Default"
        );
    }

    #[test]
    fn test_find_import_default_export_default() {
        let tsm = tsm_with_code(r#" import a from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            "ReExport from ./a.js by Default"
        );
    }
    #[test]
    fn test_find_import_named_export_default() {
        let tsm = tsm_with_code(r#" import {a} from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    #[test]
    fn test_find_import_named_renamed_export_default() {
        let tsm = tsm_with_code(r#" import {z as a} from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("z", None)"#
        );
    }

    #[test]
    fn test_find_import_namespace_export_default() {
        let tsm = tsm_with_code(r#" import * as a from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert!(re_export_source.is_none());
    }

    #[test]
    fn test_find_import_namespace_export_named() {
        let tsm = tsm_with_code(r#" import * as a from "./a.js"; export { a };"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert!(re_export_source.is_none());
    }

    #[test]
    fn test_find_import_named_export_named() {
        let tsm = tsm_with_code(r#" import { a } from "./a.js"; export { a };"#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    #[test]
    fn test_find_import_named_export_renamed() {
        let tsm = tsm_with_code(r#" import { a } from "./a.js"; export { a as b };"#);

        let re_export_source = tsm.find_export_define_wip(&"b".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    #[test]
    fn test_find_import_renamed_export_renamed() {
        let tsm = tsm_with_code(r#" import { a as b } from "./a.js"; export { b as c };"#);

        let re_export_source = tsm.find_export_define_wip(&"c".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    #[test]
    fn test_find_export_default_from() {
        let tsm = tsm_with_code(r#" export { default }  from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Default"#
        );
    }

    #[test]
    fn test_find_export_default_as_from() {
        let tsm = tsm_with_code(r#" export { default as a }  from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Default"#
        );
    }

    #[test]
    fn test_find_export_named_from() {
        let tsm = tsm_with_code(r#" export { a }  from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    #[test]
    fn test_find_export_named_as_from() {
        let tsm = tsm_with_code(r#" export { b as a }  from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("b", None)"#
        );
    }

    #[test]
    fn test_find_export_star_as_from() {
        let tsm = tsm_with_code(r#" export * as a from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert!(re_export_source.is_none());
    }

    #[test]
    fn test_find_export_star_from() {
        let mut tsm = tsm_with_code(r#" export * from "./a.js" "#);

        let re_export_source = tsm.find_export_define_wip(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a", None)"#
        );
    }

    fn tsm_with_code(code: &str) -> TreeShakeModule {
        let context: Arc<Context> = Default::default();

        let module_graph = context.module_graph.write().unwrap();

        let ast = build_js_ast("test.js", code, &context).unwrap();

        let mako_module = Module {
            id: "test.js".into(),
            is_entry: false,
            info: Some(ModuleInfo {
                ast: ModuleAst::Script(ast),
                path: "test".to_string(),
                external: None,
                raw: "".to_string(),
                raw_hash: 0,
                missing_deps: Default::default(),
                ignored_deps: vec![],
                top_level_await: false,
                is_async: false,
                resolved_resource: None,
            }),
            side_effects: false,
        };

        let tsm = GLOBALS.set(&context.meta.script.globals, || {
            TreeShakeModule::new(&mako_module, 0, module_graph.deref())
        });

        tsm
    }
}
