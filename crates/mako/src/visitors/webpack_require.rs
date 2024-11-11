use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{Expr, UnaryExpr};
use swc_core::ecma::visit::VisitMut;

use crate::ast::utils::is_ident_undefined;

const WEBPACK_REQUIRE_IDENT: &str = "__webpack_require__";

pub struct WebpackRequire {
    pub unresolved_mark: Mark,
}

impl WebpackRequire {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self { unresolved_mark }
    }
}

impl VisitMut for WebpackRequire {
    // find the "typeof __webpack_require__" in the ast tree
    fn visit_mut_unary_expr(&mut self, unary_expr: &mut UnaryExpr) {
        if unary_expr.op.as_str() == "typeof"
            && let Some(arg_ident) = unary_expr.arg.as_ident()
            && is_ident_undefined(arg_ident, WEBPACK_REQUIRE_IDENT, &self.unresolved_mark)
        {
            unary_expr.arg = Expr::undefined(DUMMY_SP)
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::WebpackRequire;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_webpack_require_ident() {
        assert_eq!(
            run(r#"typeof __webpack_require__ === 'function';"#),
            r#"typeof void 0 === 'function';"#
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = WebpackRequire {
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
