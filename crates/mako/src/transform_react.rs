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
    let use_refresh = is_dev && !task.path.contains("/node_modules/");

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
const RefreshRuntime = require( 'react-refresh');
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
import * as  RefreshRuntime from 'react-refresh';
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
