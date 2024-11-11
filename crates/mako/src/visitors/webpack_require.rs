use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{Expr, Ident, UnaryExpr};
use swc_core::ecma::visit::VisitMut;

const WEBPACK_REQUIRE: &str = "__webpack_require__";
const WEBPACK_HASH: &str = "__webpack_hash__";
const WEBPACK_LAYER: &str = "__webpack_layer__";
const WEBPACK_PUBLIC_PATH: &str = "__webpack_public_path__";
const WEBPACK_MODULES: &str = "__webpack_modules__";
const WEBPACK_MODULE: &str = "__webpack_module__";
const WEBPACK_CHUNK_LOAD: &str = "__webpack_chunk_load__";
const WEBPACK_BASE_URI: &str = "__webpack_base_uri__";
const NON_WEBPACK_REQUIRE: &str = "__non_webpack_require__";
const SYSTEM_CONTEXT: &str = "__system_context__";
const WEBPACK_SHARE_SCOPES: &str = "__webpack_share_scopes__";
const WEBPACK_INIT_SHARING: &str = "__webpack_init_sharing__";
const WEBPACK_NONCE: &str = "__webpack_nonce__";
const WEBPACK_CHUNK_NAME: &str = "__webpack_chunkname__";
const WEBPACK_RUNTIME_ID: &str = "__webpack_runtime_id__";
const WEBPACK_GET_SCRIPT_FILENAME: &str = "__webpack_get_script_filename__";

pub struct WebpackRequire {
    pub unresolved_mark: Mark,
}

impl WebpackRequire {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self { unresolved_mark }
    }
    fn is_ident_webpack(&self, ident: &Ident, unresolved_mark: &Mark) -> bool {
        [
            WEBPACK_REQUIRE,
            WEBPACK_HASH,
            WEBPACK_LAYER,
            WEBPACK_PUBLIC_PATH,
            WEBPACK_MODULES,
            WEBPACK_MODULE,
            WEBPACK_CHUNK_LOAD,
            WEBPACK_BASE_URI,
            NON_WEBPACK_REQUIRE,
            SYSTEM_CONTEXT,
            WEBPACK_SHARE_SCOPES,
            WEBPACK_INIT_SHARING,
            WEBPACK_NONCE,
            WEBPACK_CHUNK_NAME,
            WEBPACK_RUNTIME_ID,
            WEBPACK_GET_SCRIPT_FILENAME,
        ]
        .iter()
        .any(|&i| i == &ident.sym)
            && ident.ctxt.outer() == *unresolved_mark
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
