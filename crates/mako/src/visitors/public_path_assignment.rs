use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{AssignExpr, AssignOp, Pat, PatOrExpr};
use swc_core::ecma::utils::member_expr;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub struct PublicPathAssignment {
    pub unresolved_mark: Mark,
}

impl VisitMut for PublicPathAssignment {
    fn visit_mut_assign_expr(&mut self, n: &mut AssignExpr) {
        if n.op == AssignOp::Assign {
            if let PatOrExpr::Pat(box Pat::Ident(ident)) = &n.left {
                let sym = ident.sym.as_ref();
                if ident.span.ctxt.outer() == self.unresolved_mark
                    && (sym == "__webpack_public_path__" || sym == "__mako_public_path__")
                {
                    *n = AssignExpr {
                        left: PatOrExpr::Expr(member_expr!(DUMMY_SP, __mako_require__.publicPath)),
                        ..n.clone()
                    };
                }
            }
        }
        n.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::PublicPathAssignment;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(
            run(r#"__webpack_public_path__ = '/foo/';"#),
            r#"__mako_require__.publicPath = '/foo/';"#.trim()
        );
        assert_eq!(
            run(r#"__mako_public_path__ = '/foo/';"#),
            r#"__mako_require__.publicPath = '/foo/';"#.trim()
        );
    }

    #[test]
    fn test_ident_defined() {
        assert_eq!(
            run(r#"let __webpack_public_path__ = 1; __webpack_public_path__ = '/foo/';"#),
            r#"
let __webpack_public_path__ = 1;
__webpack_public_path__ = '/foo/';
"#
            .trim()
        );
        assert_eq!(
            run(r#"let __mako_public_path__ = 1; __mako_public_path__ = '/foo/';"#),
            r#"
let __mako_public_path__ = 1;
__mako_public_path__ = '/foo/';
"#
            .trim()
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = PublicPathAssignment {
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
