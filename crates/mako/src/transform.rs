use std::collections::HashMap;
use std::sync::Arc;
use swc_atoms::JsWord;
use swc_common::collections::AHashMap;
use swc_common::comments::{NoopComments, SingleThreadedComments};
use swc_common::sync::Lrc;
use swc_common::{Globals, DUMMY_SP};
use swc_common::{Mark, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_visit::VisitMutWith;
use swc_ecma_ast::{Expr, Lit, Module, Str};
use swc_ecma_preset_env::{self as swc_preset_env};
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::common_js;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::{Config, ImportInterop};
use swc_ecma_transforms::react::{react, Options};
use swc_ecma_transforms::typescript::strip_with_jsx;
use swc_ecma_transforms::{fixer, resolver, Assumptions};
use swc_ecma_visit::{Fold, VisitMutWith as CssVisitMutWith};

use crate::build::ModuleDeps;
use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::transform_css_handler::CssHandler;
use crate::transform_dep_replacer::DepReplacer;
use crate::transform_dynamic_import::DynamicImport;
use crate::transform_env_replacer::EnvReplacer;
use crate::transform_optimizer::Optimizer;

pub fn transform(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    get_deps: &mut dyn for<'r> FnMut(&'r ModuleAst) -> ModuleDeps,
) {
    match ast {
        ModuleAst::Script(ast) => transform_js(ast, context, get_deps),
        ModuleAst::Css(ast) => transform_css(ast, context, get_deps),
        _ => {}
    }
}

fn build_env_map(env_map: HashMap<String, String>) -> AHashMap<JsWord, Expr> {
    let mut map = AHashMap::default();
    env_map.into_iter().for_each(|(k, v)| {
        map.insert(
            k.into(),
            Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                raw: None,
                value: v.into(),
            })),
        );
    });
    map
}

fn transform_js(
    ast: &mut Module,
    context: &Arc<Context>,
    get_deps: &mut dyn for<'r> FnMut(&'r ModuleAst) -> ModuleDeps,
) {
    let cm = context.meta.script.cm.clone();
    let globals = Globals::default();
    // build env map
    // TODO: read env from .env
    let mode = &context.config.mode.to_string();
    let env_map = build_env_map(HashMap::from([("NODE_ENV".into(), mode.into())]));

    GLOBALS.set(&globals, || {
        let helpers = Helpers::new(true);
        HELPERS.set(&helpers, || {
            let top_level_mark = Mark::new();
            let unresolved_mark = Mark::new();
            let import_interop = ImportInterop::Swc;

            ast.visit_mut_with(&mut react(
                cm.clone(),
                Some(NoopComments),
                Options {
                    import_source: Some("react".to_string()),
                    pragma: Some("React.createElement".into()),
                    pragma_frag: Some("React.Fragment".into()),
                    ..Default::default()
                },
                top_level_mark,
                unresolved_mark,
            ));
            ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
            ast.visit_mut_with(&mut import_analyzer(import_interop, true));
            ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

            let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
            ast.visit_mut_with(&mut env_replacer);

            let mut optimizer = Optimizer {};
            ast.visit_mut_with(&mut optimizer);

            // TODO: polyfill
            let mut preset_env = swc_preset_env::preset_env(
                unresolved_mark,
                Some(NoopComments),
                swc_preset_env::Config {
                    mode: Some(swc_preset_env::Mode::Entry),
                    targets: Some(context.config.targets.clone()),
                    ..Default::default()
                },
                Assumptions::default(),
                &mut FeatureFlag::default(),
            );
            ast.body = preset_env.fold_module(ast.clone()).body;

            // 在 cjs 执行前调用 hook，用于收集依赖
            let deps = get_deps(&ModuleAst::Script(ast.clone()));

            ast.visit_mut_with(&mut common_js::<SingleThreadedComments>(
                unresolved_mark,
                Config {
                    import_interop: Some(import_interop),
                    // NOTE: 这里后面要调整为注入自定义require
                    ignore_dynamic: true,
                    preserve_import_meta: true,
                    ..Default::default()
                },
                FeatureFlag::empty(),
                None,
            ));
            ast.visit_mut_with(&mut strip_with_jsx(
                cm,
                Default::default(),
                NoopComments,
                top_level_mark,
            ));
            ast.visit_mut_with(&mut hygiene_with_config(
                swc_ecma_transforms::hygiene::Config {
                    top_level_mark,
                    ..Default::default()
                },
            ));
            ast.visit_mut_with(&mut fixer(None));

            let dep_map = get_dep_map(deps);
            let mut dep_replacer = DepReplacer { dep_map };
            ast.visit_mut_with(&mut dep_replacer);

            let mut dynamic_import = DynamicImport {};
            ast.visit_mut_with(&mut dynamic_import);
        });
    });
}

fn transform_css(
    ast: &mut Stylesheet,
    _context: &Arc<Context>,
    get_deps: &mut dyn for<'r> FnMut(&'r ModuleAst) -> ModuleDeps,
) {
    let dep_map = get_dep_map(get_deps(&ModuleAst::Css(ast.clone())));
    // remove @import and handle url()
    let mut css_handler = CssHandler { dep_map };
    ast.visit_mut_with(&mut css_handler);
}

fn get_dep_map(deps: ModuleDeps) -> HashMap<String, String> {
    deps.into_iter()
        .map(|(path, _, dep)| (dep.source, path))
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex, RwLock},
    };

    use crate::{
        ast::{build_css_ast, build_js_ast, css_ast_to_code, js_ast_to_code},
        build::ModuleDeps,
        chunk_graph::ChunkGraph,
        compiler::{Context, Meta},
        config::Config,
        module::{Dependency, ResolveType},
        module_graph::ModuleGraph,
    };

    use super::{transform_css, transform_js};

    #[test]
    fn test_react() {
        let code = r#"
const App = () => <><h1>Hello World</h1></>;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(code, r#"
const App = ()=>React.createElement(React.Fragment, null, React.createElement("h1", null, "Hello World"));

//# sourceMappingURL=index.js.map
        "#.trim());
    }

    #[test]
    fn test_strip_type() {
        let code = r#"
const Foo: string = "foo";
        "#
        .trim();
        let (code, _) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const Foo = "foo";

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_import() {
        let code = r#"
import { foo } from './foo';
        "#
        .trim();
        let (code, _) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _foo = require("./foo");

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_dynamic_import() {
        let code = r#"
const foo = import('./foo');
        "#
        .trim();
        let (code, _) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const foo = require.ensure([
    './foo'
]).then(require.bind(require, './foo'));

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_import_deps() {
        let code = r#"
import React from 'react';
        "#
        .trim();
        let (code, _) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
var _react = _interop_require_default._(require("react"));

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_js_env_replacer() {
        let code = r#"
const a = process.env.NODE_ENV;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const a = "development";

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_optimizer() {
        let code = r#"
if ('a1' === 'a1') 1.1;
if ('a2' == 'a3') 1.2;
if ('b1' !== 'b1') 2.1;
if ('b2' != 'b3') 2.2;
if ('a1' === "a2") { 3.1; } else 3.2;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
1.1;
2.2;
3.2;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_preset_env() {
        let code = r#"
const b = window.a?.b;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
var _window_a;
const b = (_window_a = window.a) === null || _window_a === void 0 ? void 0 : _window_a.b;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_dep_replacer() {
        let code = r#"
require("foo");
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
require("bar");

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_url() {
        let code = r#"
@import "should_be_removed.css";
.foo { background: url("url.png"); }
        "#
        .trim();
        let deps = Vec::from([(
            "replace.png".to_string(),
            None,
            Dependency {
                source: "url.png".to_string(),
                resolve_type: ResolveType::Css,
                order: 0,
            },
        )]);
        let code = transform_css_code(code, None, deps);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
.foo {
  background: url("replace.png");
}
        "#
            .trim()
        );
    }

    #[allow(dead_code)]
    fn test_parse_error() {
        // TODO
    }

    fn transform_js_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(..) = path {
            path.unwrap()
        } else {
            "test.tsx"
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
        let mut ast = build_js_ast(path, origin, &context);
        transform_js(&mut ast, &context, &mut |_| {
            if origin.contains("require(\"foo\");") {
                Vec::from([(
                    "bar".to_string(),
                    None,
                    Dependency {
                        source: "foo".to_string(),
                        resolve_type: ResolveType::Require,
                        order: 0,
                    },
                )])
            } else {
                Vec::new()
            }
        });
        let (code, _sourcemap) = js_ast_to_code(&ast, &context, "index.js");
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }

    fn transform_css_code(origin: &str, path: Option<&str>, deps: ModuleDeps) -> String {
        let path = if let Some(..) = path {
            path.unwrap()
        } else {
            "test.css"
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
        let mut ast = build_css_ast(path, origin, &context);
        transform_css(&mut ast, &context, &mut |_| deps.clone());
        let code = css_ast_to_code(&ast);

        code.trim().to_string()
    }
}
