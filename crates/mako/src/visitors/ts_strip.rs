use swc_core::common::util::take::Take;
use swc_core::common::Mark;
use swc_core::ecma::ast::{Module, Program};
use swc_core::ecma::transforms::typescript::strip;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub struct TypescriptStrip {
    top_level_mark: Mark,
    unresolved_mark: Mark,
}

impl VisitMut for TypescriptStrip {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let mut p = Program::Module(n.take());
        p.visit_mut_with(&mut strip(self.unresolved_mark, self.top_level_mark));

        *n = p.module().unwrap();
    }
}

pub fn ts_strip(unresolved_mark: Mark, top_level_mark: Mark) -> impl VisitMut {
    TypescriptStrip {
        top_level_mark,
        unresolved_mark,
    }
}
