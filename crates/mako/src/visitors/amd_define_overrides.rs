use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{as_folder, Fold, VisitMut};

use crate::ast::utils::is_ident_undefined;

pub struct AmdDefineOverrides {
    unresolved_mark: Mark,
}

pub fn amd_define_overrides(unresolved_mark: Mark) -> impl VisitMut + Fold {
    as_folder(AmdDefineOverrides { unresolved_mark })
}

impl VisitMut for AmdDefineOverrides {
    fn visit_mut_unary_expr(&mut self, node: &mut UnaryExpr) {
        if node.op == UnaryOp::TypeOf
            && let Some(arg_ident) = node.arg.as_ident()
            && is_ident_undefined(arg_ident, "define", &self.unresolved_mark)
        {
            node.arg = Expr::undefined(DUMMY_SP)
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn unresolve_typeof_define_change_to_undefined() {
        let mut tu = TestUtils::gen_js_ast(
            r#"if(typeof define ==="function" && define.amd) {
          console.log("amd") 
        }"#,
        );
        let js = tu.ast.js_mut();
        let unresolved_mark = js.unresolved_mark;
        GLOBALS.set(&tu.context.meta.script.globals, || {
            js.ast
                .visit_mut_with(&mut amd_define_overrides(unresolved_mark));
        });

        let code = tu.js_ast_to_code();

        assert_eq!(
            code,
            r#"if (typeof void 0 === "function" && define.amd) {
    console.log("amd");
}"#
        )
    }

    #[test]
    fn id_define_is_declared() {
        let mut tu = TestUtils::gen_js_ast(
            r#"let define = 1; if(typeof define ==="number") {
          console.log("number") 
        }"#,
        );
        let js = tu.ast.js_mut();
        let unresolved_mark = js.unresolved_mark;
        GLOBALS.set(&tu.context.meta.script.globals, || {
            js.ast
                .visit_mut_with(&mut amd_define_overrides(unresolved_mark));
        });

        let code = tu.js_ast_to_code();

        assert_eq!(
            code,
            r#"let define = 1;
if (typeof define === "number") {
    console.log("number");
}"#
        );
    }
}
