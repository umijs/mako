use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{Expr, Ident, UnaryExpr};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub const WEBPACK_VALUES: [&str; 14] = [
    "__webpack_get_script_filename__",
    "__webpack_runtime_id__",
    "__webpack_chunkname__",
    "__webpack_nonce__",
    "__webpack_init_sharing__",
    "__webpack_share_scopes__",
    "__system_context__",
    "__non_webpack_require__",
    "__webpack_require__",
    "__webpack_hash__",
    "__webpack_modules__",
    "__webpack_module__",
    "__webpack_chunk_load__",
    "__webpack_base_uri__",
];

pub struct WebpackRequire {
    pub unresolved_mark: Mark,
}

impl WebpackRequire {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self { unresolved_mark }
    }
    fn is_ident_webpack(&self, ident: &Ident, unresolved_mark: &Mark) -> bool {
        WEBPACK_VALUES.iter().any(|&i| i == &ident.sym) && ident.ctxt.outer() == *unresolved_mark
    }
}

impl VisitMut for WebpackRequire {
    // find the "typeof __webpack_require__" in the ast tree
    fn visit_mut_unary_expr(&mut self, unary_expr: &mut UnaryExpr) {
        if unary_expr.op.as_str() == "typeof"
            && let Some(arg_ident) = unary_expr.arg.as_ident()
            && self.is_ident_webpack(arg_ident, &self.unresolved_mark)
        {
            unary_expr.arg = Expr::undefined(DUMMY_SP)
        } else {
            unary_expr.visit_mut_children_with(self);
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

    #[test]
    fn test_webpack_module_ident() {
        assert_eq!(run(r#"typeof __webpack_modules__;"#), r#"typeof void 0;"#);
    }
    #[test]
    fn test_dbcheck_webpack_module_ident() {
        assert_eq!(
            run(r#"typeof typeof __webpack_modules__;"#),
            r#"typeof typeof void 0;"#
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
