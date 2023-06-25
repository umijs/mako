use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use swc_ecma_ast::Module;
use tracing::debug;

use crate::ast::{base64_encode, build_js_ast, css_ast_to_code};
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode};
use crate::module::{ModuleAst, ModuleId};
use crate::targets;

impl Compiler {
    pub fn transform_all(&self) {
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        debug!("module ids: {:?}", module_ids);
        transform_modules(module_ids, context);
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let dep_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
            .collect();
        drop(module_graph);

        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;
        if let ModuleAst::Css(ast) = ast {
            let ast = transform_css(ast, &path, dep_map, context);
            info.set_ast(ModuleAst::Script(ast));
        }
    });
}

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Module {
    // ast to code
    let (code, sourcemap) = css_ast_to_code(ast, context);

    // lightingcss
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
        let (code, _sourcemap) = js_ast_to_code(&ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
