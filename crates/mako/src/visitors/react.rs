use std::sync::Arc;

use swc_core::common::sync::Lrc;
use swc_core::common::{chain, Mark, SourceMap, Span, DUMMY_SP};
use swc_core::ecma::ast::Module;
use swc_core::ecma::transforms::react::{react as swc_react, Options, RefreshOptions, Runtime};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::{Mode, ReactRuntimeConfig};

pub fn react(
    cm: Lrc<SourceMap>,
    context: Arc<Context>,
    use_refresh: bool,
    top_level_mark: &Mark,
    unresolved_mark: &Mark,
) -> Box<dyn VisitMut> {
    let origin_comments = context.meta.script.origin_comments.read().unwrap();
    let visit = swc_react(
        cm,
        Some(origin_comments.get_swc_comments().clone()),
        Options {
            import_source: Some(context.config.react.import_source.clone()),
            pragma: Some(context.config.react.pragma.clone()),
            pragma_frag: Some(context.config.react.pragma_frag.clone()),
            runtime: Some(
                if matches!(context.config.react.runtime, ReactRuntimeConfig::Automatic) {
                    Runtime::Automatic
                } else {
                    Runtime::Classic
                },
            ),
            development: Some(matches!(context.config.mode, Mode::Development)),
            // to avoid throw error for svg namespace element
            throw_if_namespace: Some(false),
            refresh: if use_refresh {
                Some(RefreshOptions::default())
            } else {
                None
            },
            ..Default::default()
        },
        *top_level_mark,
        *unresolved_mark,
    );
    if use_refresh {
        Box::new(chain!(
            visit,
            react_refresh_module_prefix(&context),
            react_refresh_module_postfix(&context)
        ))
    } else {
        Box::new(visit)
    }
}

pub struct CleanSpan;

impl VisitMut for CleanSpan {
    fn visit_mut_span(&mut self, n: &mut Span) {
        *n = DUMMY_SP;
    }
}

struct PrefixCode {
    code: String,
    context: Arc<Context>,
}

impl VisitMut for PrefixCode {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let mut ast = JsAst::build(
            "_mako_internal/hmr_pre_code.js",
            &self.code,
            self.context.clone(),
        )
        .unwrap();

        // the sourcemap of prefix code will be duplicated when using chunk_pot::str_impl,
        // need to clean spans
        ast.ast.visit_mut_with(&mut CleanSpan {});

        module.body.splice(0..0, ast.ast.body);

        module.visit_mut_children_with(self);
    }
}

struct PostfixCode {
    code: String,
    context: Arc<Context>,
}

impl VisitMut for PostfixCode {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let mut ast = JsAst::build(
            "_mako_internal/hmr_post_code.js",
            &self.code,
            self.context.clone(),
        )
        .unwrap();

        // the sourcemap of postfix code will be duplicated when using chunk_pot::str_impl,
        // need to clean spans
        ast.ast.visit_mut_with(&mut CleanSpan {});

        module.body.extend(ast.ast.body);

        module.visit_mut_children_with(self);
    }
}

fn react_refresh_module_prefix(context: &std::sync::Arc<Context>) -> Box<dyn VisitMut> {
    let mut code = r#"
import * as RefreshRuntime from 'react-refresh';
var prevRefreshReg;
var prevRefreshSig;

prevRefreshReg = self.$RefreshReg$;
prevRefreshSig = self.$RefreshSig$;
self.$RefreshReg$ = (type, id) => {
  RefreshRuntime.register(type, module.id + id);
};
self.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
"#
    .to_string();

    // check react hmr ability if react was externalized in development mode
    if context.config.mode == Mode::Development
        && context
            .config
            .externals
            .keys()
            .any(|x| x == "react" || x == "react-dom")
    {
        code = format!(
            r#"
if (!(typeof window !== 'undefined' ? window : globalThis).__REACT_DEVTOOLS_GLOBAL_HOOK__) {{
  console.warn('HMR is not available for React currently! Because React was externalized, please install the React Developer Tools extension to enable React Refresh feature. https://github.com/pmmmwh/react-refresh-webpack-plugin/blob/main/docs/TROUBLESHOOTING.md#externalising-react');
}}
{}
        "#,
            code
        );
    }

    Box::new(PrefixCode {
        context: context.clone(),
        code,
    })
}

fn react_refresh_module_postfix(context: &Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PostfixCode {
        context: context.clone(),
        // why add `if (prevRefreshReg)` guard?
        // ref: https://github.com/umijs/mako/issues/971
        code: r#"
if (prevRefreshReg) self.$RefreshReg$ = prevRefreshReg;
if (prevRefreshSig) self.$RefreshSig$ = prevRefreshSig;
function $RefreshIsReactComponentLike$(moduleExports) {
  if (RefreshRuntime.isLikelyComponentType(moduleExports.default || moduleExports)) {
    return true;
  }
  for (var key in moduleExports) {
    try{
      if (RefreshRuntime.isLikelyComponentType(moduleExports[key])) {
        return true;
      }
    }catch(e){
       // in case the moduleExports[key] is not accessible due depedence loop
    }
  }
  return false;
}
if ($RefreshIsReactComponentLike$(module.exports)) {
    module.meta.hot.accept();
    RefreshRuntime.performReactRefresh();
}
"#
        .to_string(),
    })
}

#[cfg(test)]
mod tests {

    use swc_core::common::{Mark, GLOBALS};
    use swc_core::ecma::visit::VisitMutWith;

    use super::react;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_use_refresh() {
        let code = run("console.log('entry');", true);
        assert!(code.contains("self.$RefreshSig$ = RefreshRuntime."));
        assert!(code.contains("if (prevRefreshReg) self.$RefreshReg$ = prevRefreshReg;"));
    }

    #[test]
    fn test_jsx() {
        let code = run("function Foo() { return <>foo</> }", false);
        assert_eq!(
            code,
            r#"
import { jsxDEV as _jsxDEV, Fragment as _Fragment } from "react/jsx-dev-runtime";
function Foo() {
    return /*#__PURE__*/ _jsxDEV(_Fragment, {
        children: "foo"
    }, void 0, false);
}
        "#
            .trim()
        );
    }

    #[test]
    fn test_svgr() {
        // ref: jsoneditor/dist/img/jsoneditor-icons.svg
        let code = run(
            r#"
const Foo = () => (
    <svg
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:cc="http://creativecommons.org/ns#"
        xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
        xmlns:svg="http://www.w3.org/2000/svg"
        xmlns="http://www.w3.org/2000/svg"
        xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
        xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
        id="svg4136"
        inkscape:version="0.91 r13725"
        sodipodi:docname="jsoneditor-icons.svg"
        {...props}
    >
        <metadata id="metadata4148">
            <rdf:RDF></rdf:RDF>
        </metadata>
    </svg>
)
        "#,
            false,
        );
        println!("{}", code);
        // no panic means it's ok
    }

    fn run(js_code: &str, use_refresh: bool) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = react(
                Default::default(),
                test_utils.context.clone(),
                use_refresh,
                &Mark::new(),
                &Mark::new(),
            );
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
