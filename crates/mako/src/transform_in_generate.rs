use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use std::collections::HashMap;
use std::sync::Arc;
use swc_css_visit::VisitMutWith as CssVisitMutWith;
use swc_ecma_ast::Module;
use tracing::{debug, info};

use crate::ast::{build_js_ast, css_ast_to_code};
use crate::compiler::Context;
use crate::module::ModuleId;
use crate::{compiler::Compiler, module::ModuleAst, transform_css_handler::CssHandler};

impl Compiler {
    pub fn transform_all(&self) {
        info!("transform all modules");
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
        match ast {
            ModuleAst::Css(ast) => {
                let ast = transform_css(ast, &path, dep_map, context);
                info.set_ast(ModuleAst::Script(ast));
            }
            _ => {}
        }
    });
}

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Module {
    // remove @import and handle url()
    let mut css_handler = CssHandler {
        dep_map: dep_map.clone(),
    };
    ast.visit_mut_with(&mut css_handler);

    // ast to code
    let code = css_ast_to_code(ast);

    // lightingcss
    let mut lightingcss_stylesheet = StyleSheet::parse(&code, ParserOptions::default()).unwrap();
    lightingcss_stylesheet
        .minify(MinifyOptions::default())
        .unwrap();
    let out = lightingcss_stylesheet
        .to_css(PrinterOptions::default())
        .unwrap();
    let code = out.code.as_str();

    // code to js ast
    let content = include_str!("runtime/runtime_css.ts").to_string();
    let content = content.replace("__CSS__", code);
    let require_code: Vec<String> = dep_map
        .values()
        .filter(|val| val.ends_with(".css"))
        .map(|val| format!("require(\"{}\");", val))
        .collect();
    let content = format!("{}{}", require_code.join("\n"), content);
    let path = format!("{}.ts", path);
    let path = path.as_str();
    build_js_ast(path, content.as_str(), context)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex, RwLock},
    };

    use crate::{
        ast::{build_css_ast, js_ast_to_code},
        chunk_graph::ChunkGraph,
        compiler::{Context, Meta},
        config::Config,
        module_graph::ModuleGraph,
    };

    use super::transform_css;

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
`;
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
@import "./foo.css";
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
`;
let style = document.createElement('style');
style.innerHTML = css;
document.head.appendChild(style);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_url() {
        let code = r#"
.foo { background: url("url.png"); }
        "#
        .trim();
        let (code, _cm) = transform_css_code(
            code,
            None,
            HashMap::from([("url.png".into(), "replace.png".into())]),
        );
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
let css = `.foo {
  background: url("replace.png");
}
`;
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
        let path = if path.is_none() {
            "test.tsx"
        } else {
            path.unwrap()
        };
        let root = PathBuf::from("/path/to/root");
        let context = Arc::new(Context {
            config: Config::new(&root).unwrap(),
            root,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            meta: Meta::new(),
        });
        let mut ast = build_css_ast(path, content, &context);
        let ast = transform_css(&mut ast, path, dep_map, &context);
        let (code, _sourcemap) = js_ast_to_code(&ast, &context, "index.js");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
