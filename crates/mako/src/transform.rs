use std::sync::Arc;

use anyhow::Result;
use swc_common::comments::NoopComments;
use swc_common::errors::HANDLER;
use swc_common::sync::Lrc;
use swc_common::{chain, Mark, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_visit::VisitMutWith;
use swc_ecma_ast::Module;
use swc_ecma_preset_env::{self as swc_preset_env};
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::ImportInterop;
use swc_ecma_transforms::proposals::decorators;
use swc_ecma_transforms::typescript::strip_with_jsx;
use swc_ecma_transforms::{resolver, Assumptions};
use swc_ecma_visit::{Fold, VisitMutWith as CssVisitMutWith};
use swc_error_reporters::handler::try_with_handler;

use crate::build::Task;
use crate::compiler::Context;
use crate::config::Mode;
use crate::module::ModuleAst;
use crate::resolve::Resolvers;
use crate::targets;
use crate::transform_css_url_replacer::CSSUrlReplacer;
use crate::transform_env_replacer::{build_env_map, EnvReplacer};
use crate::transform_optimizer::Optimizer;
use crate::transform_provide::Provide;
use crate::transform_react::mako_react;

pub fn transform(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    task: &Task,
    resolvers: &Resolvers,
) -> Result<()> {
    match ast {
        ModuleAst::Script(ast) => transform_js(
            &mut ast.ast,
            context,
            task,
            ast.top_level_mark,
            ast.unresolved_mark,
        ),
        ModuleAst::Css(ast) => transform_css(ast, context, task, resolvers),
        _ => Ok(()),
    }
}

fn transform_js(
    ast: &mut Module,
    context: &Arc<Context>,
    task: &Task,
    top_level_mark: Mark,
    unresolved_mark: Mark,
) -> Result<()> {
    let cm = context.meta.script.cm.clone();
    let mode = &context.config.mode.to_string();
    let mut define = context.config.define.clone();

    define
        .entry("NODE_ENV".to_string())
        .or_insert_with(|| format!("\"{}\"", mode).into());
    let _is_dev = matches!(context.config.mode, Mode::Development);

    let env_map = build_env_map(define, context)?;
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    let import_interop = ImportInterop::Swc;

                    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
                    ast.visit_mut_with(&mut strip_with_jsx(
                        cm.clone(),
                        Default::default(),
                        NoopComments,
                        top_level_mark,
                    ));

                    // ast.visit_mut_with(&mut dce(
                    //     DceConfig {
                    //         top_level: false,
                    //         ..Default::default()
                    //     },
                    //     unresolved_mark,
                    // ));
                    //
                    // indent.span needed in mako_react refresh, so it must be after resolver visitor
                    ast.visit_mut_with(&mut mako_react(
                        cm,
                        context,
                        task,
                        &top_level_mark,
                        &unresolved_mark,
                    ));

                    ast.visit_mut_with(&mut import_analyzer(import_interop, true));

                    let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
                    ast.visit_mut_with(&mut env_replacer);

                    let mut provide = Provide::new(context.config.providers.clone());
                    ast.visit_mut_with(&mut provide);

                    let mut optimizer = Optimizer {};
                    ast.visit_mut_with(&mut optimizer);

                    // TODO: polyfill
                    let preset_env = swc_preset_env::preset_env(
                        unresolved_mark,
                        Some(NoopComments),
                        swc_preset_env::Config {
                            mode: Some(swc_preset_env::Mode::Entry),
                            targets: Some(targets::swc_preset_env_targets_from_map(
                                context.config.targets.clone(),
                            )),
                            ..Default::default()
                        },
                        Assumptions::default(),
                        &mut FeatureFlag::default(),
                    );
                    let mut folders = chain!(
                        preset_env,
                        // support decorator
                        // TODO: support config
                        decorators(decorators::Config {
                            legacy: true,
                            emit_metadata: false,
                            ..Default::default()
                        })
                    );
                    ast.body = folders.fold_module(ast.clone()).body;

                    // inject helpers must after decorators
                    // since decorators will use helpers
                    ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                    Ok(())
                })
            })
        })
    })
}

fn transform_css(
    ast: &mut Stylesheet,
    context: &Arc<Context>,
    task: &Task,
    resolvers: &Resolvers,
) -> Result<()> {
    let mut css_handler = CSSUrlReplacer {
        resolvers,
        path: &task.path,
        context,
    };
    ast.visit_mut_with(&mut css_handler);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, RwLock};

    use super::transform_js;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::chunk::{Chunk, ChunkType};
    use crate::chunk_graph::ChunkGraph;
    use crate::compiler::{Context, Meta};
    use crate::config::Config;
    use crate::module::ModuleId;
    use crate::module_graph::ModuleGraph;
    use crate::transform_dep_replacer::DependenciesToReplace;
    use crate::transform_in_generate::{transform_js_generate, TransformJsParam};

    #[test]
    fn test_react() {
        let code = r#"
const App = () => <><h1>Hello World</h1></>;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _jsxdevruntime = require("react/jsx-dev-runtime");
const App = ()=>(0, _jsxdevruntime.jsxDEV)(_jsxdevruntime.Fragment, {
        children: (0, _jsxdevruntime.jsxDEV)("h1", {
            children: "Hello World"
        }, void 0, false, {
            fileName: "test.tsx",
            lineNumber: 1,
            columnNumber: 21
        }, this)
    }, void 0, false);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_strip_type() {
        let code = r#"
const Foo: string = "foo";
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
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
    fn test_strip_type_2() {
        let code = r#"
import { X } from 'foo';
import x from 'foo';
x;
const b: X;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
var _foo = _interop_require_default._(require("foo"));
_foo.default;
const b;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_import() {
        let code = r#"
import { foo } from './foo';
foo;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _foo = require("./foo");
_foo.foo;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_import_2() {
        let code = r#"
import * as foo from './foo';
foo.bar;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Object.defineProperty(exports, "__esModule", {
    value: true
});
var _interop_require_wildcard = require("@swc/helpers/_/_interop_require_wildcard");
var _foo = _interop_require_wildcard._(require("./foo"));
_foo.bar;

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
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const foo = Promise.all([
    require.ensure("./foo")
]).then(require.bind(require, "./foo"));

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_provide() {
        let code = r#"
console.log(process);
console.log(process.env);
Buffer.from('foo');
function foo() {
    let process = 1;
    console.log(process);
    let Buffer = 'b';
    Buffer.from('foo');
}
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
console.log(require("process"));
console.log(require("process").env);
require("buffer").Buffer.from('foo');
function foo() {
    let process = 1;
    console.log(process);
    let Buffer = 'b';
    Buffer.from('foo');
}

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
        let (code, _) = transform_js_code(code, None, HashMap::new());
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
const EXIT = false;
console.log(EXIT);
console.log(FOO);
if (1) {
  const FOO = 1;
  console.log(FOO);
}
console.log(PACKAGE_NAME);
const a = process.env.NODE_ENV;
const b = process.env.PACKAGE_NAME;
const c = MEMBERS;
const d = YOUYOU.name;
const e = XIAOHUONI.friend;
const f = MEMBER_NAMES;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const EXIT = false;
console.log(EXIT);
console.log(false);
if (1) {
    const FOO = 1;
    console.log(FOO);
}
console.log("MAKO");
const a = "development";
const b = "MAKO";
const c = 3;
const d = {
    name: "youyou"
}.name;
const e = {
    friend: {
        name: "sorrycc"
    }
}.friend;
const f = [
    {
        name: "sorrycc"
    },
    {
        name: "xiaohuoni"
    }
];

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
        let (code, _sourcemap) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
1.1;
;
;
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
        let (code, _sourcemap) = transform_js_code(code, None, HashMap::new());
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
        let (code, _sourcemap) = transform_js_code(
            code,
            None,
            HashMap::from([("foo".to_string(), "./bar".to_string())]),
        );
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
require("./bar");

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn transform_js_code(
        origin: &str,
        path: Option<&str>,
        dep: HashMap<String, String>,
    ) -> (String, String) {
        let path = path.unwrap_or("test.tsx");
        let current_dir = std::env::current_dir().unwrap();
        let mut config = Config::new(&current_dir.join("test/config/define"), None, None).unwrap();
        // for test_provider
        config
            .providers
            .insert("process".into(), ("process".into(), "".into()));
        config
            .providers
            .insert("Buffer".into(), ("buffer".into(), "Buffer".into()));

        let root = PathBuf::from("/path/to/root");

        let mut chunk_graph = ChunkGraph::new();
        chunk_graph.add_chunk(Chunk::new("./foo".to_string().into(), ChunkType::Async));

        let context = Arc::new(Context {
            config,
            args: Default::default(),
            root: root.clone(),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(chunk_graph),
            assets_info: Mutex::new(HashMap::new()),
            modules_with_missing_deps: RwLock::new(Vec::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
            stats_info: Mutex::new(Default::default()),
        });

        let mut ast = build_js_ast(path, origin, &context).unwrap();
        transform_js(
            &mut ast.ast,
            &context,
            &crate::build::Task {
                path: root.join(path).to_string_lossy().to_string(),
                parent_resource: None,
                is_entry: false,
            },
            ast.top_level_mark,
            ast.unresolved_mark,
        )
        .unwrap();
        transform_js_generate(TransformJsParam {
            _id: &ModuleId::new("test".to_string()),
            context: &context,
            ast: &mut ast,
            dep_map: &DependenciesToReplace {
                resolved: dep,
                missing: HashMap::new(),
            },
            async_deps: &vec![],
            is_entry: false,
            is_async: false,
            top_level_await: false,
        });
        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
