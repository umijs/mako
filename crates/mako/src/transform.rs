use swc_common::comments::{NoopComments, SingleThreadedComments};
use swc_common::{sync::Lrc, Globals};
use swc_common::{Mark, SourceMap, GLOBALS};
use swc_ecma_ast::Module;
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::common_js;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::{Config, ImportInterop};
use swc_ecma_transforms::react::{react, Options};
use swc_ecma_transforms::typescript::strip_with_jsx;
use swc_ecma_transforms::{fixer, resolver};
use swc_ecma_visit::VisitMutWith;

use crate::module::ModuleAst;

pub fn transform(ast: &mut ModuleAst, cm: &Lrc<SourceMap>) {
    match ast {
        ModuleAst::Script(ast) => transform_js(ast, cm),
        _ => {}
    }
}

// TODO:
// polyfill and targets
fn transform_js(ast: &mut Module, cm: &Lrc<SourceMap>) {
    let globals = Globals::default();
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
            ast.visit_mut_with(&mut common_js::<SingleThreadedComments>(
                unresolved_mark,
                Config {
                    import_interop: Some(import_interop),
                    // NOTE: 这里后面要调整为注入自定义require
                    // ignore_dynamic: true,
                    preserve_import_meta: true,
                    ..Default::default()
                },
                FeatureFlag::empty(),
                None,
            ));
            ast.visit_mut_with(&mut strip_with_jsx(
                cm.clone(),
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
    use crate::ast::{build_js_ast, js_ast_to_code};

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
const foo = Promise.resolve().then(function() {
    return _interop_require_wildcard(require("./foo"));
});
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

    #[allow(dead_code)]
    fn test_parse_error() {
        // TODO
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if path.is_none() {
            "test.tsx"
        } else {
            path.unwrap()
        };
        let (cm, mut ast) = build_js_ast(path, origin);
        transform_js(&mut ast, &cm);
        let (code, _sourcemap) = js_ast_to_code(&ast, &cm);
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
