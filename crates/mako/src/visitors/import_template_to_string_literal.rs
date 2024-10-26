use swc_core::ecma::ast::{CallExpr, Callee, Expr};
use swc_core::ecma::utils::quote_str;
use swc_core::ecma::visit::VisitMut;

pub struct ImportTemplateToStringLiteral {}

impl VisitMut for ImportTemplateToStringLiteral {
    fn visit_mut_call_expr(&mut self, n: &mut CallExpr) {
        if matches!(n.callee, Callee::Import(_)) && n.args.len() == 1 {
            if let box Expr::Tpl(tpl) = &n.args[0].expr {
                if tpl.exprs.is_empty() && tpl.quasis.len() == 1 {
                    let s: String = tpl.quasis[0].raw.to_string();

                    n.args[0].expr = quote_str!(tpl.span, s).into();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(run(r#"import(`a`)"#), r#"import("a");"#);
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = super::ImportTemplateToStringLiteral {};
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
