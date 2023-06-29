use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use swc_common::errors::HANDLER;
use swc_common::GLOBALS;
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::fixer;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::common_js;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::{Config, ImportInterop};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;
use tracing::debug;

use crate::ast::{base64_encode, build_js_ast, css_ast_to_code, Ast};
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode};
use crate::module::{ModuleAst, ModuleId};
use crate::targets;
use crate::transform_dep_replacer::DepReplacer;
use crate::transform_dynamic_import::DynamicImport;

impl Compiler {
    pub fn transform_all(&self) -> Result<()> {
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        debug!("module ids: {:?}", module_ids);
        transform_modules(module_ids, context)?;
        Ok(())
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let dep_map: HashMap<String, String> = deps
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
                                    // let unresolved_mark = Mark::new();
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
                                                .comments
                                                .read()
                                                .unwrap()
                                                .get_swc_comments(),
                                        ),
                                    ));

                                    let mut dynamic_import = DynamicImport {};
                                    ast.ast.visit_mut_with(&mut dynamic_import);

                                    let mut dep_replacer = DepReplacer {
                                        dep_map: dep_map.clone(),
                                    };
                                    ast.ast.visit_mut_with(&mut dep_replacer);

                                    ast.ast.visit_mut_with(&mut hygiene_with_config(
                                        swc_ecma_transforms::hygiene::Config {
                                            top_level_mark,
                                            ..Default::default()
                                        },
                                    ));
                                    ast.ast.visit_mut_with(&mut fixer(None));
                                    Ok(())
                                })
                            })
                        },
                    )
                })
                .unwrap();
        }

        if let ModuleAst::Css(ast) = ast {
            let ast = transform_css(ast, &path, dep_map, context);
            info.set_ast(ModuleAst::Script(ast));
        }
    });
    Ok(())
}

// fn get_dep_map(deps: Vec<(&ModuleId, &crate::module::Dependency)>) -> HashMap<String, String> {
//     deps.into_iter()
//         .map(|(.., dep)| (dep.source.clone(), dep.replaced_source.unwrap().clone()))
//         .collect::<HashMap<_, _>>()
// }

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Ast {
    // ast to code
    let (code, sourcemap) = css_ast_to_code(ast, context);

    // lightingcss
    // something more, lightning will transform @import url() to @import ""
    let targets = targets::lightningcss_targets_from_map(context.config.targets.clone());
    let mut lightingcss_stylesheet = StyleSheet::parse(&code, ParserOptions::default()).unwrap();
    lightingcss_stylesheet
        .minify(MinifyOptions {
            targets,
            ..Default::default()
        })
        .unwrap();
    let out = lightingcss_stylesheet
        .to_css(PrinterOptions {
            minify: matches!(context.config.mode, Mode::Production),
            targets,
            ..Default::default()
        })
        .unwrap();
    let mut code = out.code;

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
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, RwLock};

    use super::transform_css;
    use crate::ast::{build_css_ast, js_ast_to_code};
    use crate::chunk_graph::ChunkGraph;
    use crate::compiler::{Context, Meta};
    use crate::module_graph::ModuleGraph;

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
        let root = PathBuf::from("/path/to/root");
        let context = Arc::new(Context {
            config: Default::default(),
            root,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            meta: Meta::new(),
        });
        let mut ast = build_css_ast(path, content, &context).unwrap();
        let ast = transform_css(&mut ast, path, dep_map, &context);
        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
