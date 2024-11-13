use swc_core::common::SyntaxContext;
use swc_core::ecma::visit::{as_folder, Fold, VisitMut};

struct CleanSyntaxContext;

pub fn clean_syntax_context() -> impl VisitMut + Fold {
    as_folder(CleanSyntaxContext {})
}

impl VisitMut for CleanSyntaxContext {
    fn visit_mut_syntax_context(&mut self, ctxt: &mut SyntaxContext) {
        *ctxt = SyntaxContext::empty();
    }
}
