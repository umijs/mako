use std::collections::HashMap;
use std::sync::Arc;
use swc_atoms::JsWord;
use swc_common::collections::AHashMap;
use swc_common::comments::{NoopComments, SingleThreadedComments};
use swc_common::sync::Lrc;
use swc_common::{Globals, DUMMY_SP};
use swc_common::{Mark, GLOBALS};
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
use swc_ecma_visit::{Fold, VisitMutWith};

use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::transform_env_replacer::EnvReplacer;
use crate::transform_optimizer::Optimizer;

pub fn transform(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    analyze_deps_hook: &mut dyn for<'r> FnMut(&'r ModuleAst),
) {
    match ast {
        ModuleAst::Script(ast) => transform_js(ast, context, analyze_deps_hook),
        _ => {
            analyze_deps_hook(ast);
        }
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
    before_cjs_hook: &mut dyn for<'r> FnMut(&'r ModuleAst),
) {
    let cm = context.meta.script.cm.clone();
    let globals = Globals::default();
    // build env map
    let env_map = build_env_map(HashMap::from([("NODE_ENV".into(), "production".into())]));

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
            before_cjs_hook(&ModuleAst::Script(ast.clone()));

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
        });
    });
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex, RwLock},
    };

    use crate::{
        ast::{build_js_ast, js_ast_to_code},
        chunk_graph::ChunkGraph,
        compiler::{Context, Meta},
        config::Config,
        module_graph::ModuleGraph,
    };

    use super::transform_js;

    #[test]
    fn test_react() {
        let code = r#"
const App = () => <><h1>Hello World</h1></>;
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(code, r#"
const App = ()=>React.createElement(React.Fragment, null, React.createElement("h1", null, "Hello World"));
        "#.trim());
    }

    #[test]
    fn test_strip_type() {
        let code = r#"
const Foo: string = "foo";
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const Foo = "foo";
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
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _foo = require("./foo");
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
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const foo = import('./foo');
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
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
var _react = _interop_require_default._(require("react"));
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
        let (code, _sourcemap) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const a = "production";
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
        let (code, _sourcemap) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
1.1;
2.2;
3.2;
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
        let (code, _sourcemap) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
var _window_a;
const b = (_window_a = window.a) === null || _window_a === void 0 ? void 0 : _window_a.b;
        "#
            .trim()
        );
    }

    #[allow(dead_code)]
    fn test_parse_error() {
        // TODO
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
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
        transform_js(&mut ast, &context, &mut |_| {});
        let (code, _sourcemap) = js_ast_to_code(&ast, &context, "index.js");
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
