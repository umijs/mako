use std::sync::Arc;

use swc_common::comments::NoopComments;
use swc_common::sync::Lrc;
use swc_common::{chain, Mark, SourceMap};
use swc_ecma_ast::Module;
use swc_ecma_transforms::react::{react, Options, RefreshOptions, Runtime};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast::build_js_ast;
use crate::build::Task;
use crate::compiler::Context;
use crate::config::Mode;

pub struct PrefixCode {
    code: String,
    context: Arc<Context>,
}

pub fn mako_react(
    cm: Lrc<SourceMap>,
    context: &Arc<Context>,
    task: &Task,
    top_level_mark: &Mark,
    unresolved_mark: &Mark,
) -> Box<dyn VisitMut> {
    let is_dev = matches!(context.config.mode, Mode::Development);
    let use_refresh = is_dev && context.config.hmr && !task.path.contains("/node_modules/");

    let visit = react(
        cm,
        Some(NoopComments),
        Options {
            import_source: Some("react".to_string()),
            pragma: Some("React.createElement".into()),
            pragma_frag: Some("React.Fragment".into()),
            // support react 17 + only
            runtime: Some(Runtime::Automatic),
            development: Some(is_dev),
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
        Box::new(if task.is_entry {
            chain!(visit, react_refresh_entry_prefix(context), noop())
        } else {
            chain!(
                visit,
                react_refresh_module_prefix(context),
                react_refresh_module_postfix(context)
            )
        })
    } else {
        Box::new(visit)
    }
}

impl VisitMut for PrefixCode {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let post_code_snippet_module =
            build_js_ast("_pre_code.js", &self.code, &self.context).unwrap();
        module.body.splice(0..0, post_code_snippet_module.body);

        module.visit_mut_children_with(self);
    }
}

pub struct PostfixCode {
    code: String,
    context: Arc<Context>,
}

impl VisitMut for PostfixCode {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let post_code_snippet_module =
            build_js_ast("_post_code.js", &self.code, &self.context).unwrap();
        module.body.extend(post_code_snippet_module.body);

        module.visit_mut_children_with(self);
    }
}

pub fn react_refresh_entry_prefix(context: &Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
const RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(window)
window.$RefreshReg$ = () => {}
window.$RefreshSig$ = () => (type) => type
"#
        .to_string(),
    })
}

pub fn react_refresh_module_prefix(context: &std::sync::Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
import * as RefreshRuntime from 'react-refresh';
var prevRefreshReg;
var prevRefreshSig;

prevRefreshReg = window.$RefreshReg$;
prevRefreshSig = window.$RefreshSig$;
window.$RefreshReg$ = (type, id) => {
  RefreshRuntime.register(type, module.id + id);
};
window.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
"#
        .to_string(),
    })
}

pub fn react_refresh_module_postfix(context: &Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PostfixCode {
        context: context.clone(),
        code: r#"
window.$RefreshReg$ = prevRefreshReg;
window.$RefreshSig$ = prevRefreshSig;
module.meta.hot.accept();
RefreshRuntime.performReactRefresh();
"#
        .to_string(),
    })
}

struct NoopVisitor;

impl VisitMut for NoopVisitor {
    fn visit_mut_module(&mut self, _: &mut Module) {
        // Do nothing.
    }
}

fn noop() -> Box<dyn VisitMut> {
    Box::new(NoopVisitor)
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use swc_common::sync::Lrc;
    use swc_common::{chain, Globals, Mark, SourceMap, GLOBALS};
    use swc_ecma_ast::Module;
    use swc_ecma_codegen::text_writer::JsWriter;
    use swc_ecma_codegen::Emitter;
    use swc_ecma_transforms::resolver;
    use swc_ecma_visit::VisitMutWith;

    use crate::ast::build_js_ast;
    use crate::build::Task;
    use crate::compiler::Context;
    use crate::transform_react::mako_react;

    #[test]
    pub fn entry_with_react_refresh() {
        assert_eq!(
            r#"const RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(window);
window.$RefreshReg$ = ()=>{};
window.$RefreshSig$ = ()=>(type)=>type;
console.log('entry');"#,
            transform(TransformTask {
                is_entry: true,
                path: "index.js".to_string(),
                code: "console.log('entry');".to_string()
            })
        );
    }

    #[test]
    pub fn node_modules_with_react_refresh() {
        assert_eq!(
            r#"console.log('in node modules');"#,
            transform(TransformTask {
                code: "console.log('in node modules');".to_string(),
                is_entry: false,
                path: "project/node_modules/pkg/index.js".to_string()
            })
        );
    }

    struct TransformTask {
        code: String,
        path: String,
        is_entry: bool,
    }

    #[test]
    pub fn normal_module_with_react_refresh() {
        assert_eq!(
            r#"import * as RefreshRuntime from 'react-refresh';
var prevRefreshReg;
var prevRefreshSig;
prevRefreshReg = window.$RefreshReg$;
prevRefreshSig = window.$RefreshSig$;
window.$RefreshReg$ = (type, id)=>{
    RefreshRuntime.register(type, module.id + id);
};
window.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
import { jsxDEV as _jsxDEV } from "react/jsx-dev-runtime";
export default function R() {
    return _jsxDEV("h1", {}, void 0, false, {
        fileName: "<<jsx-config-pragmaFrag.js>>",
        lineNumber: 1,
        columnNumber: 16
    }, this);
}
_c = R;
var _c;
$RefreshReg$(_c, "R");
window.$RefreshReg$ = prevRefreshReg;
window.$RefreshSig$ = prevRefreshSig;
module.meta.hot.accept();
RefreshRuntime.performReactRefresh();"#,
            transform(TransformTask {
                code: "export default function R(){return <h1></h1>}".to_string(),
                is_entry: false,
                path: "index.js".to_string()
            })
        );
    }

    fn transform(task: TransformTask) -> String {
        let context: Arc<Context> = Arc::new(Default::default());

        let globals = Globals::new();
        GLOBALS.set(&globals, || {
            // Your code here
            let mut ast = build_js_ast("index.jsx", &task.code, &context).unwrap();

            let unresolved_mark = Mark::new();
            let top_level_mark = Mark::new();

            let mut visitor = chain!(
                resolver(unresolved_mark, top_level_mark, false),
                mako_react(
                    Default::default(),
                    &context,
                    &Task {
                        is_entry: task.is_entry,
                        path: task.path,
                    },
                    &Mark::new(),
                    &Mark::new(),
                )
            );

            ast.visit_mut_with(&mut visitor);
            emit_js(&ast)
        })
    }

    fn emit_js(module: &Module) -> String {
        let cm: Lrc<SourceMap> = Default::default();
        let mut buf = Vec::new();

        {
            let writer = Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None));
            let mut emitter = Emitter {
                cfg: Default::default(),
                comments: None,
                cm,
                wr: writer,
            };
            // This may return an error if it fails to write
            emitter.emit_module(module).unwrap();
        }

        String::from_utf8(buf).unwrap().trim().to_string()
    }
}
