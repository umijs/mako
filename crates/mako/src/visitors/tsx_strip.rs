use std::sync::Arc;

use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::util::take::Take;
use swc_core::common::Mark;
use swc_core::ecma::ast::{Module, Program};
use swc_core::ecma::transforms::typescript;
use swc_core::ecma::transforms::typescript::TsxConfig;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;

pub struct TsxStrip {
    cm: Arc<swc_core::common::SourceMap>,
    context: Arc<Context>,
    top_level_mark: Mark,
    unresolved_mark: Mark,
}

impl VisitMut for TsxStrip {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let comments = SingleThreadedComments::default();
        let tsx_config = TsxConfig {
            pragma: Some(self.context.config.react.pragma.clone()),
            pragma_frag: Some(self.context.config.react.pragma_frag.clone()),
        };
        let mut p = Program::Module(n.take());
        p.visit_mut_with(&mut typescript::tsx(
            self.cm.clone(),
            Default::default(),
            tsx_config,
            comments,
            self.unresolved_mark,
            self.top_level_mark,
        ));
        *n = p.module().unwrap();
    }
}

pub fn tsx_strip(
    cm: Arc<swc_core::common::SourceMap>,
    context: Arc<Context>,
    top_level_mark: Mark,
    unresolved_mark: Mark,
) -> impl VisitMut {
    TsxStrip {
        cm,
        context,
        top_level_mark,
        unresolved_mark,
    }
}
