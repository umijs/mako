use mako_core::anyhow;
use mako_core::swc_atoms::JsWord;
use mako_core::swc_ecma_ast::{
    ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier, Module, ModuleDecl,
    ModuleExportName, ModuleItem, Str,
};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::config::{TransformImportConfig, TransformImportStyle};
use crate::plugin::Plugin;

pub struct ImportVisitor<'a> {
    config: &'a Vec<TransformImportConfig>,
}

impl<'a> ImportVisitor<'a> {
    fn find_import_config(&self, src: &str) -> Option<&TransformImportConfig> {
        self.config.iter().find(|config| config.library_name == src)
    }
}

impl<'a> VisitMut for ImportVisitor<'a> {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let mut cur = 0;

        while module.body.len() > cur {
            // visit all top-level import statements
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(decl)) = &mut module.body[cur] {
                let members = &decl
                    .specifiers
                    .iter()
                    .filter(|s| {
                        // normal named import
                        // ex. import { Button } from 'antd';
                        matches!(
                            s,
                            ImportSpecifier::Named(ImportNamedSpecifier { imported: None, .. })
                        ) ||
                        // alias named import but not string
                        // ex. import { Button as MyButton } from 'antd';
                        matches!(
                            s,
                            ImportSpecifier::Named(ImportNamedSpecifier {
                                imported: Some(ModuleExportName::Ident(_)),
                                ..
                            })
                        )
                    })
                    .collect::<Vec<_>>();

                // skip if no matched config or no member imports
                if let (Some(import_config), Some(_)) =
                    (self.find_import_config(&decl.src.value), members.first())
                {
                    let library_dir = import_config
                        .library_directory
                        .clone()
                        .unwrap_or_else(|| "lib".to_string());
                    let members = &members
                        .iter()
                        .map(|s| match s {
                            ImportSpecifier::Named(n) => n,
                            _ => unreachable!(),
                        })
                        .collect::<Vec<_>>();
                    let mut expanded_imports = vec![];

                    for member in members {
                        // expand member exports
                        let mut member_stmt = decl.clone();
                        let imported = match &member.imported {
                            Some(imported) => match imported {
                                ModuleExportName::Ident(n) => &n.sym,
                                _ => unreachable!(),
                            },
                            None => &member.local.sym,
                        };
                        let member_src = format!(
                            "{}/{}/{}",
                            decl.src.value,
                            library_dir,
                            // CamelCase to kebab-case
                            imported
                                .to_string()
                                .chars()
                                .fold(String::new(), |mut acc, c| {
                                    if c.is_uppercase() {
                                        if acc.len() > 1 {
                                            acc.push('-');
                                        }
                                        acc.push(c.to_ascii_lowercase());
                                    } else {
                                        acc.push(c);
                                    }
                                    acc
                                })
                        );
                        let member_specifier = ImportDefaultSpecifier {
                            span: member.span,
                            local: member.local.clone(),
                        };

                        member_stmt.specifiers.clear();
                        member_stmt
                            .specifiers
                            .push(ImportSpecifier::Default(member_specifier));
                        *member_stmt.src = Str {
                            value: JsWord::from(member_src.clone()),
                            span: member_stmt.src.span,
                            raw: None,
                        };
                        expanded_imports
                            .push(ModuleItem::ModuleDecl(ModuleDecl::Import(member_stmt)));

                        // expend style for member exports
                        if let Some(style_config) = &import_config.style {
                            let mut style_stmt = decl.clone();
                            let mut style_src = format!("{}/style", member_src);

                            if let TransformImportStyle::Built(style) = style_config {
                                style_src = format!("{}/{}", style_src, style);
                            }

                            style_stmt.specifiers.clear();
                            *style_stmt.src = Str {
                                value: JsWord::from(style_src),
                                span: style_stmt.src.span,
                                raw: None,
                            };
                            expanded_imports
                                .push(ModuleItem::ModuleDecl(ModuleDecl::Import(style_stmt)));
                        }
                    }

                    let mut skip_count = expanded_imports.len();
                    let mut insert_index = cur + 1;

                    // replace original import statement
                    if members.len() == decl.specifiers.len() {
                        module.body.remove(cur);
                        skip_count -= 1;
                        insert_index -= 1;
                    } else {
                        decl.specifiers
                            .retain(|s| !matches!(s, ImportSpecifier::Named(_)));
                    }

                    // append expanded imports
                    module
                        .body
                        .splice(insert_index..insert_index, expanded_imports);

                    // skip loop for expanded imports
                    cur += skip_count;
                }
            }

            cur += 1;
        }
    }
}

pub struct ImportPlugin {}

impl Plugin for ImportPlugin {
    fn name(&self) -> &str {
        "import"
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        ast: &mut mako_core::swc_ecma_ast::Module,
        context: &std::sync::Arc<crate::compiler::Context>,
    ) -> anyhow::Result<()> {
        // skip node_modules to keep behavior same as umi, and skip if no config
        if param.path.contains("node_modules") || context.config.transform_import.is_empty() {
            return Ok(());
        }

        ast.visit_mut_with(&mut ImportVisitor {
            config: &context.config.transform_import,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::swc_ecma_visit::VisitMutWith;

    use crate::ast::js_ast::JsAst;
    use crate::compiler::Context;
    use crate::config::{TransformImportConfig, TransformImportStyle};
    use crate::plugins::import::ImportVisitor;

    #[test]
    fn test_multi() {
        let code = generate(
            r#"
import { Button, DatePicker } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: None,
                style: None,
            }],
        );
        assert_eq!(
            code,
            r#"
import Button from "antd/lib/button";
import DatePicker from "antd/lib/date-picker";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    #[test]
    fn test_multi_style() {
        let code = generate(
            r#"
import { Button, DatePicker } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: None,
                style: Some(TransformImportStyle::Source(true)),
            }],
        );
        assert_eq!(
            code,
            r#"
import Button from "antd/lib/button";
import "antd/lib/button/style";
import DatePicker from "antd/lib/date-picker";
import "antd/lib/date-picker/style";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    #[test]
    fn test_multi_builtin_style() {
        let code = generate(
            r#"
import { Button, DatePicker } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: None,
                style: Some(TransformImportStyle::Built("css".to_string())),
            }],
        );
        assert_eq!(
            code,
            r#"
import Button from "antd/lib/button";
import "antd/lib/button/style/css";
import DatePicker from "antd/lib/date-picker";
import "antd/lib/date-picker/style/css";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    #[test]
    fn test_multi_lib_dir() {
        let code = generate(
            r#"
import { Button, DatePicker } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: Some("es".to_string()),
                style: None,
            }],
        );
        assert_eq!(
            code,
            r#"
import Button from "antd/es/button";
import DatePicker from "antd/es/date-picker";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    #[test]
    fn test_single_named() {
        let code = generate(
            r#"
import { Button as MyButton } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: None,
                style: None,
            }],
        );
        assert_eq!(
            code,
            r#"
import MyButton from "antd/lib/button";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    #[test]
    fn test_complex() {
        let code = generate(
            r#"
import antd1 from "antd";
import * as antd2 from "antd";
import antd3, { Checkbox, Form } from "antd";
import { Button, DatePicker } from "antd";
        "#,
            &vec![TransformImportConfig {
                library_name: "antd".to_string(),
                library_directory: None,
                style: None,
            }],
        );
        assert_eq!(
            code,
            r#"
import antd1 from "antd";
import * as antd2 from "antd";
import antd3 from "antd";
import Checkbox from "antd/lib/checkbox";
import Form from "antd/lib/form";
import Button from "antd/lib/button";
import DatePicker from "antd/lib/date-picker";

//# sourceMappingURL=/test/path.map
        "#
            .trim(),
        );
    }

    fn generate(code: &str, config: &Vec<TransformImportConfig>) -> String {
        let path = "/test/path";
        let context: Arc<Context> = Arc::new(Default::default());
        let mut ast = JsAst::build(path, code, context.clone()).unwrap();
        ast.ast.visit_mut_with(&mut ImportVisitor { config });
        ast.generate(context.clone()).unwrap().code
    }
}
