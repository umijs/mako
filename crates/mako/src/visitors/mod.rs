pub(crate) mod css_dep_analyzer;
pub(crate) mod js_dep_analyzer;

use mako_core::swc_ecma_visit::Visit;
use swc_core::common::Mark;

pub(crate) fn js_mut_visitors(unresolved_mark: Mark) -> Vec<impl Visit> {
    vec![js_dep_analyzer::JSDepAnalyzer::new(unresolved_mark)]
}
