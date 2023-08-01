use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use pathdiff::diff_paths;
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

use crate::ast::{base64_encode, build_js_ast, css_ast_to_code, Ast};
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode};
use crate::module::{ModuleAst, ModuleId};
use crate::targets;
use crate::transform_css_handler::CssHandler;
use crate::transform_dep_replacer::DepReplacer;
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

        let base: PathBuf = module_id.id.clone().into();
        let base = base.parent().unwrap();

        let dep_map: HashMap<String, String> = deps
            .clone()
            .into_iter()
            // .map(|(id, dep)| (dep.source.clone(), id.generate(context)))
            .map(|(id, dep)| {
                let is_from_src = module_id.id.contains("/src/");
                let is_to_npm = id.id.contains("node_modules");

                let mut rel_path = diff_paths(&id.id, base).unwrap().with_extension("js");

                println!("from {:?} relative to {:?}", &module_id.id, &id.id);

                if is_from_src && is_to_npm {
                    dbg!(&rel_path);
                    rel_path = rel_path.strip_prefix("../").unwrap().to_path_buf()
                }

                let replacement: String = {
                    let mut to_path = rel_path.to_str().unwrap().to_string();
                    if to_path.starts_with('.') {
                        to_path
                    } else {
                        to_path.insert_str(0, "./");
                        to_path
                    }
                };

                (dep.source.clone(), replacement)
            })
            .collect();
        let assets_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
            .collect();
        drop(module_graph);

        // let deps: Vec<(&ModuleId, &crate::module::Dependency)> =
        //     module_graph.get_dependencies(module_id);
        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;

        if let ModuleAst::Script(ast) = ast {
            transform_js_generate(&module.id, context, ast, &dep_map, module.is_entry);
        }

        // 通过开关控制是否单独提取css文件
        if !context.config.extract_css {
            if let ModuleAst::Css(ast) = ast {
                let ast = transform_css(ast, &path, dep_map, assets_map, context);
                info.set_ast(ModuleAst::Script(ast));
            }
        }
    });
    Ok(())
}

pub fn transform_js_generate(
    id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &HashMap<String, String>,
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
                                dep_map: dep_map.clone(),
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

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    assets_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Ast {
    // replace deps
    let mut css_handler = CssHandler {
        assets_map,
        context,
    };
    ast.visit_mut_with(&mut css_handler);

    // prefixer
    let mut prefixer = swc_css_prefixer::prefixer(swc_css_prefixer::options::Options {
        env: Some(targets::swc_preset_env_targets_from_map(
            context.config.targets.clone(),
        )),
    });
    ast.visit_mut_with(&mut prefixer);

    // minifier
    if matches!(context.config.mode, Mode::Production) {
        swc_css_minifier::minify(ast, Default::default());
    }

    // ast to code
    let (mut code, sourcemap) = css_ast_to_code(ast, context);
    // lightingcss
    // let mut code = lightingcss_transform(&code, context);

    // TODO: 后续支持生成单独的 css 文件后需要优化
    if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
        let path_buf = PathBuf::from(path);
        let filename = path_buf.file_name().unwrap();
        fs::write(
            format!(
                "{}.map",
                context.config.output.path.join(filename).to_string_lossy()
            ),
            &sourcemap,
        )
        .unwrap_or(());
        code = format!(
            "{code}\n/*# sourceMappingURL={}.map*/",
            filename.to_string_lossy()
        );
    } else if matches!(context.config.devtool, DevtoolConfig::InlineSourceMap) {
        code = format!(
            "{code}\n/*# sourceMappingURL=data:application/json;charset=utf-8;base64,{}*/",
            base64_encode(&sourcemap)
        );
    }

    // code to js ast
    let content = include_str!("runtime/runtime_css.ts").to_string();
    let content = content.replace("__CSS__", code.as_str());
    let require_code: Vec<String> = dep_map
        .values()
        .filter(|val| val.ends_with(".css"))
        .map(|val| format!("require(\"{}\");\n", val))
        .collect();
    let content = format!("{}{}", require_code.join(""), content);
    let path = format!("{}.ts", path);
    let path = path.as_str();
    // TODO: handle error
    build_js_ast(path, content.as_str(), context).unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::transform_css;
    use crate::ast::{build_css_ast, js_ast_to_code};

    #[test]
    fn test_transform_css() {
        let code = r#"
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None, Default::default());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
let css = `.foo {
  color: red;
}
/*# sourceMappingURL=test.tsx.map*/`;
let style = document.createElement('style');
style.innerHTML = css;
document.head.appendChild(style);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_import() {
        let code = r#"
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) =
            transform_css_code(code, None, HashMap::from([("1".into(), "bar.css".into())]));
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
require("bar.css");
let css = `.foo {
  color: red;
}
/*# sourceMappingURL=test.tsx.map*/`;
let style = document.createElement('style');
style.innerHTML = css;
document.head.appendChild(style);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn transform_css_code(
        content: &str,
        path: Option<&str>,
        dep_map: HashMap<String, String>,
    ) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context = Arc::new(Default::default());
        let mut ast = build_css_ast(path, content, &context).unwrap();
        let ast = transform_css(&mut ast, path, dep_map.clone(), dep_map, &context);
        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
