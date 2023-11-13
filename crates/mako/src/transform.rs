use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::comments::NoopComments;
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{chain, Mark, GLOBALS};
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_css_visit::VisitMutWith as CssVisitMutWith;
use mako_core::swc_ecma_ast::Module;
use mako_core::swc_ecma_preset_env::{self as swc_preset_env};
use mako_core::swc_ecma_transforms::feature::FeatureFlag;
use mako_core::swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use mako_core::swc_ecma_transforms::optimization::simplifier;
use mako_core::swc_ecma_transforms::optimization::simplify::{dce, Config as SimpilifyConfig};
use mako_core::swc_ecma_transforms::proposals::decorators;
use mako_core::swc_ecma_transforms::typescript::strip_with_jsx;
use mako_core::swc_ecma_transforms::{resolver, Assumptions};
use mako_core::swc_ecma_visit::{Fold, VisitMutWith};
use mako_core::swc_error_reporters::handler::try_with_handler;

use crate::build::Task;
use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::plugin::PluginTransformJsParam;
use crate::resolve::Resolvers;
use crate::targets;
use crate::transformers::transform_css_url_replacer::CSSUrlReplacer;
use crate::transformers::transform_dynamic_import_to_require::DynamicImportToRequire;
use crate::transformers::transform_env_replacer::{build_env_map, EnvReplacer};
use crate::transformers::transform_optimize_package_imports::optimize_package_imports;
use crate::transformers::transform_provide::Provide;
use crate::transformers::transform_px2rem::Px2Rem;
use crate::transformers::transform_react::mako_react;
use crate::transformers::transform_try_resolve::TryResolve;
use crate::transformers::transform_virtual_css_modules::VirtualCSSModules;

pub fn transform(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    task: &Task,
    resolvers: &Resolvers,
) -> Result<()> {
    mako_core::mako_profile_function!();
    match ast {
        ModuleAst::Script(ast) => transform_js(
            &mut ast.ast,
            context,
            task,
            ast.top_level_mark,
            ast.unresolved_mark,
            resolvers,
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
    _resolvers: &Resolvers,
) -> Result<()> {
    let cm = context.meta.script.cm.clone();
    let mode = &context.config.mode.to_string();
    let mut define = context.config.define.clone();

    define
        .entry("NODE_ENV".to_string())
        .or_insert_with(|| format!("\"{}\"", mode).into());
    let is_ts = task.path.ends_with(".ts") || task.path.ends_with(".tsx");

    let env_map = build_env_map(define, context)?;
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
                    // strip should be ts only
                    // since when use this in js, it will remove all unused imports
                    // which is not expected as what webpack does
                    if is_ts {
                        ast.visit_mut_with(&mut strip_with_jsx(
                            cm.clone(),
                            Default::default(),
                            NoopComments,
                            top_level_mark,
                        ));
                    }

                    ast.visit_mut_with(&mut mako_react(
                        cm,
                        context,
                        task,
                        &top_level_mark,
                        &unresolved_mark,
                    ));

                    let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
                    ast.visit_mut_with(&mut env_replacer);

                    // watch 模式下全部会走 missing 然后转 throw error，无需提前转换
                    if !context.args.watch {
                        let mut try_resolve = TryResolve {
                            path: task.path.clone(),
                            context,
                            unresolved_mark,
                        };
                        ast.visit_mut_with(&mut try_resolve);
                    }

                    let mut provide = Provide::new(context.config.providers.clone());
                    ast.visit_mut_with(&mut provide);

                    let mut import_css_in_js = VirtualCSSModules {
                        context,
                        unresolved_mark,
                    };
                    ast.visit_mut_with(&mut import_css_in_js);

                    if context.config.dynamic_import_to_require {
                        let mut dynamic_import_to_require = DynamicImportToRequire {};
                        ast.visit_mut_with(&mut dynamic_import_to_require);
                    }

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
                        }),
                        // simplify, but keep top level dead code
                        // e.g. import x from 'foo'; but x is not used
                        // this must be keeped for tree shaking to work
                        simplifier(
                            unresolved_mark,
                            SimpilifyConfig {
                                dce: dce::Config {
                                    top_level: false,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        ),
                        optimize_package_imports(task.path.clone(), context.clone()),
                    );

                    ast.body = folders.fold_module(ast.clone()).body;

                    // inject helpers must after decorators
                    // since decorators will use helpers
                    ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                    // plugin transform
                    context.plugin_driver.transform_js(
                        &PluginTransformJsParam {
                            handler,
                            path: &task.path,
                            top_level_mark,
                            unresolved_mark,
                        },
                        ast,
                        context,
                    )?;

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
    if context.config.px2rem {
        let mut px2rem = Px2Rem {
            path: &task.path,
            context,
            current_decl: None,
            current_selector: None,
        };
        ast.visit_mut_with(&mut px2rem);
    }
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
    use crate::config::{hash_config, Config};
    use crate::module::ModuleId;
    use crate::module_graph::ModuleGraph;
    use crate::resolve::get_resolvers;
    use crate::transform_in_generate::{transform_js_generate, TransformJsParam};
    use crate::transformers::transform_dep_replacer::DependenciesToReplace;

    #[test]
    fn test_react() {
        let code = r#"
const App = () => <><h1>Hello World</h1></>;
App;
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
App;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_strip_type() {
        let code = r#"
const Foo: string = "foo";
Foo;
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
const Foo = "foo";
Foo;

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
const b: X = 1;
b;
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
const b = 1;
b;

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
foo;
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
foo;

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
foo();
        "#
        .trim();
        let (code, _) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        let common = r#"
console.log(process);
console.log(process.env);
Buffer.from('foo');
function foo() {
    let process = 1;
    console.log(process);
    let Buffer = 'b';
    Buffer.from('foo');
}
foo();

//# sourceMappingURL=index.js.map"#
            .trim();
        let require1 = r#"
const Buffer = require("buffer").Buffer;
const process = require("process");
        "#
        .trim();
        let require2 = r#"
const process = require("process");
const Buffer = require("buffer").Buffer;
        "#
        .trim();
        // 内部使用 RandomState hashmap，require 的顺序有两种可能
        let result = code == format!("{}\n{}", require1, common)
            || code == format!("{}\n{}", require2, common);
        assert!(result);
    }

    #[test]
    fn test_import_deps() {
        let code = r#"
import React from 'react';
React;
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
_react.default;

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
a;
b;
c;
d;
e;
f;
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
{
    const FOO = 1;
    console.log(FOO);
}console.log("MAKO");
const a = "development";
const b = "MAKO";
const c = 3;
const d = "youyou";
const e = {
    name: "sorrycc"
};
const f = [
    {
        name: "sorrycc"
    },
    {
        name: "xiaohuoni"
    }
];
a;
b;
c;
d;
e;
f;

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_preset_env() {
        let code = r#"
const b = window.a?.b;
b;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(code, None, HashMap::new());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
var _window_a;
const b = (_window_a = window.a) === null || _window_a === void 0 ? void 0 : _window_a.b;
b;

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

    #[test]
    fn test_optimize_if() {
        let code = r#"
if(1 == 1) { console.log("1"); } else { console.log("2"); }

if(1 == 2) { console.log("1"); } else if (1 == 1) { console.log("2"); } else { console.log("3"); }

if(1 == 2) { console.log("1"); } else if (1 == 2) { console.log("2"); } else { console.log("3"); }

if(null === null) { console.log("null==null optimized"); } else {"ooops"}

if(true) { console.log("1"); } else { console.log("2"); }
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
            r#"console.log("1");
console.log("2");
console.log("3");
console.log("null==null optimized");
console.log("1");

//# sourceMappingURL=index.js.map
"#
            .trim()
        );
    }

    #[test]
    fn test_non_optimize_if() {
        let code = r#"
if(1 == 'a') { "should keep" }

if(null == undefined) { "should keep" }

if(/x/ === /x/) { "should keep" }
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
            r#""should keep";
if (/x/ === /x/) "should keep";

//# sourceMappingURL=index.js.map
"#
            .trim()
        );
    }

    #[test]
    fn test_private_property_assign() {
        // test will not panic
        let code = r#"
        class A {
            #a: number;
            b() {
                this.#a ||= 1;
            }
        }
"#
        .trim();
        let (_code, _sourcemap) = transform_js_code(code, None, HashMap::from([]));
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

        let resolvers = get_resolvers(&config);
        let config_hash = hash_config(&config);

        let context = Arc::new(Context {
            config,
            config_hash,
            args: Default::default(),
            root: root.clone(),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(chunk_graph),
            assets_info: Mutex::new(HashMap::new()),
            modules_with_missing_deps: RwLock::new(Vec::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
            stats_info: Mutex::new(Default::default()),
            resolvers,
            optimize_infos: Mutex::new(None),
            static_cache: Default::default(),
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
            &context.resolvers,
        )
        .unwrap();
        transform_js_generate(TransformJsParam {
            _id: &ModuleId::new("test".to_string()),
            context: &context,
            ast: &mut ast,
            dep_map: &DependenciesToReplace {
                resolved: dep,
                missing: HashMap::new(),
                ignored: vec![],
            },
            async_deps: &vec![],
            is_entry: false,
            wrap_async: false,
            top_level_await: false,
        });
        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
