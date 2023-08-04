use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use swc_common::errors::HANDLER;
use swc_common::GLOBALS;
use swc_css_visit::VisitMutWith as CSSVisitMutWith;
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::fixer;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::common_js;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::{Config, ImportInterop};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

use crate::ast::Ast;
use crate::compiler::{Compiler, Context};
use crate::config::Mode;
use crate::module::{ModuleAst, ModuleId};
use crate::targets;
use crate::transform_css_handler::CssHandler;
use crate::transform_dep_replacer::{DepReplacer, DependenciesToReplace};
use crate::transform_dynamic_import::DynamicImport;
use crate::transform_react::react_refresh_entry_prefix;
use crate::unused_statement_sweep::UnusedStatementSweep;

impl Compiler {
    pub fn transform_all(&self) -> Result<()> {
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        transform_modules(module_ids, context)?;
        Ok(())
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let resolved_deps: HashMap<String, String> = deps
            .clone()
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.generate(context)))
            .collect();
        drop(module_graph);

        // let deps: Vec<(&ModuleId, &crate::module::Dependency)> =
        //     module_graph.get_dependencies(module_id);
        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let ast = &mut info.ast;

        let deps_to_replace = DependenciesToReplace {
            resolved: resolved_deps,
            missing: info.missing_deps.clone(),
        };

        if let ModuleAst::Script(ast) = ast {
            transform_js_generate(&module.id, context, ast, &deps_to_replace, module.is_entry);
        }
    });
    Ok(())
}

pub fn transform_js_generate(
    id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &DependenciesToReplace,
    is_entry: bool,
) {
    let is_dev = matches!(context.config.mode, Mode::Development);
    GLOBALS
        .set(&context.meta.script.globals, || {
            try_with_handler(
                context.meta.script.cm.clone(),
                Default::default(),
                |handler| {
                    HELPERS.set(&Helpers::new(true), || {
                        HANDLER.set(handler, || {
                            let unresolved_mark = ast.unresolved_mark;
                            let top_level_mark = ast.top_level_mark;
                            // let (code, ..) = js_ast_to_code(&ast.ast, context, "foo").unwrap();
                            // print!("{}", code);
                            {
                                if context.config.minify
                                    && matches!(context.config.mode, Mode::Production)
                                {
                                    let comments =
                                        context.meta.script.output_comments.read().unwrap();
                                    let mut unused_statement_sweep =
                                        UnusedStatementSweep::new(id, &comments);
                                    ast.ast.visit_mut_with(&mut unused_statement_sweep);
                                }
                            }

                            let import_interop = ImportInterop::Swc;
                            // FIXME: 执行两轮 import_analyzer + inject_helpers，第一轮是为了 module_graph，第二轮是为了依赖替换
                            ast.ast
                                .visit_mut_with(&mut import_analyzer(import_interop, true));
                            ast.ast.visit_mut_with(&mut inject_helpers(unresolved_mark));
                            ast.ast.visit_mut_with(&mut common_js(
                                unresolved_mark,
                                Config {
                                    import_interop: Some(import_interop),
                                    // NOTE: 这里后面要调整为注入自定义require
                                    ignore_dynamic: true,
                                    preserve_import_meta: true,
                                    // TODO: 在 esm 时设置为 false
                                    allow_top_level_this: true,
                                    ..Default::default()
                                },
                                FeatureFlag::empty(),
                                Some(
                                    context
                                        .meta
                                        .script
                                        .origin_comments
                                        .read()
                                        .unwrap()
                                        .get_swc_comments(),
                                ),
                            ));

                            if is_entry && is_dev {
                                ast.ast
                                    .visit_mut_with(&mut react_refresh_entry_prefix(context));
                            }

                            let mut dep_replacer = DepReplacer {
                                to_replace: dep_map,
                                context,
                            };
                            ast.ast.visit_mut_with(&mut dep_replacer);

                            let mut dynamic_import = DynamicImport { context };
                            ast.ast.visit_mut_with(&mut dynamic_import);

                            ast.ast.visit_mut_with(&mut hygiene_with_config(
                                swc_ecma_transforms::hygiene::Config {
                                    top_level_mark,
                                    ..Default::default()
                                },
                            ));
                            ast.ast.visit_mut_with(&mut fixer(Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            )));

                            Ok(())
                        })
                    })
                },
            )
        })
        .unwrap();
}

pub fn transform_css_generate(ast: &mut swc_css_ast::Stylesheet, context: &Arc<Context>) {
    // replace deps
    let mut css_handler = CssHandler {};
    ast.visit_mut_with(&mut css_handler);

    // prefixer
    let mut prefixer = swc_css_prefixer::prefixer(swc_css_prefixer::options::Options {
        env: Some(targets::swc_preset_env_targets_from_map(
            context.config.targets.clone(),
        )),
    });
    ast.visit_mut_with(&mut prefixer);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::transform_css_generate;
    use crate::ast::{build_css_ast, css_ast_to_code};

    #[test]
    fn test_transform_css_import() {
        let code = r#"
@import "./bar.css";
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#".foo {
  color: red;
}"#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_import_hoist() {
        let code = r#"
@import "https://example.com/first.css";
.foo { color: red; }
@import "https://example.com/foo.css";
.bar { color: blue; }
@import "https://example.com/bar.css";
.other { color: green; }
@import "https://example.com/other.css";
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"@import "https://example.com/first.css";
@import "https://example.com/foo.css";
@import "https://example.com/bar.css";
@import "https://example.com/other.css";
.foo {
  color: red;
}
.bar {
  color: blue;
}
.other {
  color: green;
}"#
            .trim()
        );
    }

    fn transform_css_code(content: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context = Arc::new(Default::default());
        let mut ast = build_css_ast(path, content, &context).unwrap();
        transform_css_generate(&mut ast, &context);
        let (code, _sourcemap) = css_ast_to_code(&ast, &context);
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
