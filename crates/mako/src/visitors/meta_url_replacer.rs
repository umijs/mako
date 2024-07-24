use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{CondExpr, Expr};
use swc_core::ecma::utils::member_expr;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::is_import_meta_url;

pub struct MetaUrlReplacer {}

impl VisitMut for MetaUrlReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if is_import_meta_url(expr) {
            // Compatible with workers: self.document ? self.document.baseURI : self.location.href
            *expr = Expr::Cond(CondExpr {
                span: DUMMY_SP,
                test: member_expr!(DUMMY_SP, self.document).into(),
                cons: member_expr!(DUMMY_SP, self.document.baseURI).into(),
                alt: member_expr!(DUMMY_SP, self.location.href).into(),
            });
        }

        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::MetaUrlReplacer;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(
            run("import.meta.url"),
            "self.document ? self.document.baseURI : self.location.href;"
        )
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = MetaUrlReplacer {};
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
