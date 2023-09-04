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
    pub code: String,
    pub context: Arc<Context>,
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

    let is_jsx =
        task.path.ends_with(".jsx") || task.path.ends_with(".tsx") || task.path.ends_with(".svg");

    if !is_jsx {
        return if task.is_entry && use_refresh {
            Box::new(chain!(react_refresh_inject_runtime_only(context), noop()))
        } else {
            Box::new(noop())
        };
    }

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
            chain!(
                visit,
                react_refresh_module_prefix(context),
                react_refresh_module_postfix(context)
            )
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
        module.body.splice(0..0, post_code_snippet_module.ast.body);

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
        module.body.extend(post_code_snippet_module.ast.body);

        module.visit_mut_children_with(self);
    }
}

pub fn react_refresh_entry_prefix(context: &Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
const RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(window);
window.$RefreshReg$ = () => {};
window.$RefreshSig$ = () => (type) => type;
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

pub fn react_refresh_inject_runtime_only(context: &std::sync::Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
import 'react-refresh';
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

    use swc_common::{chain, Mark, GLOBALS};
    use swc_ecma_transforms::resolver;
    use swc_ecma_visit::VisitMut;

    use crate::assert_display_snapshot;
    use crate::ast::build_js_ast;
    use crate::build::Task;
    use crate::compiler::Context;
    use crate::test_helper::transform_ast_with;
    use crate::transform_react::mako_react;

    struct TransformTask {
        code: String,
        path: String,
        is_entry: bool,
    }

    #[test]
    pub fn entry_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            is_entry: true,
            path: "index.js".to_string(),
            code: "console.log('entry');".to_string()
        }));
    }

    #[test]
    pub fn node_modules_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            code: "console.log('in node modules');".to_string(),
            is_entry: false,
            path: "project/node_modules/pkg/index.js".to_string()
        }));
    }

    #[test]
    pub fn normal_module_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            code: "export default function R(){return <h1></h1>}".to_string(),
            is_entry: false,
            path: "index.jsx".to_string()
        }));
    }

    fn transform(task: TransformTask) -> String {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("index.jsx", &task.code, &context).unwrap();

            let mut visitor: Box<dyn VisitMut> = Box::new(chain!(
                resolver(Mark::new(), Mark::new(), false),
                mako_react(
                    Default::default(),
                    &context,
                    &Task {
                        is_entry: task.is_entry,
                        path: task.path.to_string(),
                        parent_resource: None,
                    },
                    &Mark::new(),
                    &Mark::new(),
                )
            ));

            transform_ast_with(&mut ast.ast, &mut visitor, &context.meta.script.cm)
        })
    }
}
