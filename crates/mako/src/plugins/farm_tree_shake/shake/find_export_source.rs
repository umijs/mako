use crate::plugins::farm_tree_shake::module::{is_ident_sym_equal, TreeShakeModule};
use crate::plugins::farm_tree_shake::shake::skip_module::{ReExportSource, ReExportType};
use crate::plugins::farm_tree_shake::shake::strip_context;
use crate::plugins::farm_tree_shake::statement_graph::{ExportSpecifierInfo, ImportSpecifierInfo};

impl TreeShakeModule {
    pub fn find_skipable_export_source(&self, ident: &String) -> Option<ReExportSource> {
        let mut local_ident = None;
        let mut re_export_type = None;

        let mut ambiguous_named = vec![];

        for stmt in self.stmt_graph.stmts() {
            if let Some(export_info) = &stmt.export_info {
                if let Some(export_specifier) = export_info.find_export_specifier(ident) {
                    if let Some(source) = &export_info.source {
                        match export_specifier {
                            ExportSpecifierInfo::All(all_exports) => {
                                if all_exports.iter().any(|i| is_ident_sym_equal(i, ident)) {
                                    return Some(ReExportSource {
                                        re_export_type: ReExportType::Named(strip_context(ident)),
                                        source: Some(source.clone()),
                                    });
                                }
                            }
                            ExportSpecifierInfo::Ambiguous(all_exports) => {
                                let reexport_source = ReExportSource {
                                    re_export_type: ReExportType::Named(strip_context(ident)),
                                    source: Some(source.clone()),
                                };

                                if all_exports.iter().any(|i| is_ident_sym_equal(i, ident)) {
                                    return Some(reexport_source);
                                } else {
                                    ambiguous_named.push(reexport_source);
                                }
                            }
                            ExportSpecifierInfo::Named { exported, local } => {
                                let stripped_local = strip_context(local);

                                if let Some(exported_name) = exported {
                                    if is_ident_sym_equal(exported_name, ident) {
                                        return Some(ReExportSource {
                                            re_export_type: if stripped_local == "default" {
                                                ReExportType::Default
                                            } else {
                                                ReExportType::Named(stripped_local.clone())
                                            },
                                            source: Some(source.clone()),
                                        });
                                    }
                                } else if is_ident_sym_equal(ident, local) {
                                    return Some(ReExportSource {
                                        re_export_type: if stripped_local == "default" {
                                            ReExportType::Default
                                        } else {
                                            ReExportType::Named(stripped_local.clone())
                                        },
                                        source: Some(source.clone()),
                                    });
                                }
                            }
                            ExportSpecifierInfo::Default(_) => {
                                // Never when export with source
                                // export default from "x" is not supported in mako

                                if ident == "default" {
                                    return None;
                                }
                            }
                            // export * as x from "x"
                            ExportSpecifierInfo::Namespace(name) => {
                                let stripped_name = strip_context(name);
                                if stripped_name.eq(ident) {
                                    return Some(ReExportSource {
                                        re_export_type: ReExportType::Namespace,
                                        source: Some(source.clone()),
                                    });
                                }
                            }
                        }
                    } else {
                        match export_specifier {
                            ExportSpecifierInfo::All(_) => {}
                            ExportSpecifierInfo::Named { exported, local } => {
                                if let Some(exported_name) = exported {
                                    if is_ident_sym_equal(exported_name, ident) {
                                        re_export_type =
                                            Some(ReExportType::Named(strip_context(exported_name)));

                                        local_ident = Some(local.clone());
                                        break;
                                    }
                                } else if is_ident_sym_equal(ident, local) {
                                    re_export_type =
                                        Some(ReExportType::Named(strip_context(local)));
                                    local_ident = Some(local.clone());

                                    break;
                                }
                            }
                            ExportSpecifierInfo::Default(export_default_ident) => {
                                if ident == "default" {
                                    if let Some(default_ident) = export_default_ident {
                                        re_export_type = Some(ReExportType::Default);
                                        local_ident = Some(default_ident.clone());
                                        break;
                                    } else {
                                        return Some(ReExportSource {
                                            re_export_type: ReExportType::Default,
                                            source: None,
                                        });
                                    }
                                }
                            }
                            //  never happen when export without source
                            ExportSpecifierInfo::Namespace(_)
                            | ExportSpecifierInfo::Ambiguous(_) => {
                                return None;
                            }
                        }
                    }
                }
            }
        }

        if local_ident.is_none() && ambiguous_named.len() == 1 {
            return ambiguous_named.pop();
        }

        if let Some(local) = &local_ident {
            for stmt in self.stmt_graph.stmts() {
                if let Some(import_info) = &stmt.import_info {
                    if let Some(import_specifier) = import_info.find_define_specifier(local) {
                        match import_specifier {
                            ImportSpecifierInfo::Namespace(_namespace) => {
                                return Some(ReExportSource {
                                    re_export_type: ReExportType::Namespace,
                                    source: Some(import_info.source.clone()),
                                });
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

                                    return Some(ReExportSource {
                                        re_export_type: ReExportType::Named(strip_context(
                                            &next_name,
                                        )),
                                        source: Some(import_info.source.clone()),
                                    });
                                }
                            }
                            ImportSpecifierInfo::Default(name) => {
                                if local == name {
                                    return None;
                                }
                            }
                        }
                    }
                }
            }

            re_export_type.map(|re_export_type| ReExportSource {
                re_export_type,
                source: None,
            })
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
    use crate::ast::file::{Content, File};
    use crate::ast::js_ast::JsAst;
    use crate::compiler::Context;
    use crate::module::{Module, ModuleAst, ModuleInfo};
    use crate::plugins::farm_tree_shake::shake::skip_module::ReExportSource;

    impl ReExportSource {
        pub fn describe(&self) -> String {
            if let Some(source) = &self.source {
                format!("ReExport from {} by {:?}", source, self.re_export_type)
            } else {
                format!("Direct Export {:?}", self.re_export_type)
            }
        }
    }

    #[test]
    fn test_find_import_default_export_named() {
        let tsm = tsm_with_code(r#" import a from "./a.js"; export {a}; "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert!(re_export_source.is_none());
    }

    #[test]
    fn test_find_import_default_export_default() {
        let tsm = tsm_with_code(r#" import a from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert!(re_export_source.is_none());
    }
    #[test]
    fn test_find_import_named_export_default() {
        let tsm = tsm_with_code(r#" import {a} from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_import_named_renamed_export_default() {
        let tsm = tsm_with_code(r#" import {z as a} from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("z")"#
        );
    }

    #[test]
    fn test_find_import_namespace_export_default() {
        let tsm = tsm_with_code(r#" import * as a from "./a.js"; export default a;"#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert!(re_export_source.is_none());
    }

    #[test]
    fn test_find_import_namespace_export_named() {
        let tsm = tsm_with_code(r#" import * as a from "./a.js"; export { a };"#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Namespace"#
        );
    }

    #[test]
    fn test_find_import_named_export_named() {
        let tsm = tsm_with_code(r#" import { a } from "./a.js"; export { a };"#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_import_named_export_renamed() {
        let tsm = tsm_with_code(r#" import { a } from "./a.js"; export { a as b };"#);

        let re_export_source = tsm.find_skipable_export_source(&"b".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_import_renamed_export_renamed() {
        let tsm = tsm_with_code(r#" import { a as b } from "./a.js"; export { b as c };"#);

        let re_export_source = tsm.find_skipable_export_source(&"c".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_export_default_from() {
        let tsm = tsm_with_code(r#" export { default }  from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Default"#
        );
    }

    #[test]
    fn test_find_export_default_as_from() {
        let tsm = tsm_with_code(r#" export { default as a }  from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Default"#
        );
    }

    #[test]
    fn test_find_export_named_from() {
        let tsm = tsm_with_code(r#" export { a }  from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_export_named_as_from() {
        let tsm = tsm_with_code(r#" export { b as a }  from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("b")"#
        );
    }

    #[test]
    fn test_find_export_star_as_from() {
        let tsm = tsm_with_code(r#" export * as a from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Namespace"#
        );
    }

    #[test]
    #[ignore]
    // test in e2e
    fn test_find_export_star_from() {
        let tsm = tsm_with_code(r#" export * from "./a.js" "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"ReExport from ./a.js by Named("a")"#
        );
    }

    #[test]
    fn test_find_export_default_local_ident() {
        let tsm = tsm_with_code(r#"const a=1; export default a "#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Default"#
        );
    }

    #[test]
    fn test_find_export_default_function() {
        let tsm = tsm_with_code(r#"export default function test(){} "#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Default"#
        );
    }

    #[test]
    fn test_find_export_default_class() {
        let tsm = tsm_with_code(r#" export default class Test{} "#);

        let re_export_source = tsm.find_skipable_export_source(&"default".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Default"#
        );
    }

    #[test]
    fn test_find_export_named_class() {
        let tsm = tsm_with_code(r#" export class TestClass{} "#);

        let re_export_source = tsm.find_skipable_export_source(&"TestClass".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Named("TestClass")"#
        );
    }

    #[test]
    fn test_find_export_named_fn() {
        let tsm = tsm_with_code(r#" export function fnTest(){} "#);

        let re_export_source = tsm.find_skipable_export_source(&"fnTest".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Named("fnTest")"#
        );
    }

    #[test]
    fn test_find_export_dec_expr() {
        let tsm = tsm_with_code(r#" export const a = 1 "#);

        let re_export_source = tsm.find_skipable_export_source(&"a".to_string());

        assert_eq!(
            re_export_source.unwrap().describe(),
            r#"Direct Export Named("a")"#
        );
    }

    fn tsm_with_code(code: &str) -> TreeShakeModule {
        let context: Arc<Context> = Default::default();

        let module_graph = context.module_graph.write().unwrap();

        let file = File::with_content(
            "test.js".to_string(),
            Content::Js(code.to_string()),
            context.clone(),
        );
        let ast = JsAst::new(&file, context.clone()).unwrap();

        let mako_module = Module {
            id: "test.js".into(),
            is_entry: false,
            info: Some(ModuleInfo {
                ast: ModuleAst::Script(ast),
                path: "test".to_string(),
                file,
                ..Default::default()
            }),
            side_effects: false,
        };

        let tsm = GLOBALS.set(&context.meta.script.globals, || {
            TreeShakeModule::new(&mako_module, 0, module_graph.deref())
        });

        tsm
    }
}
