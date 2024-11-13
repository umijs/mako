use std::sync::Arc;

use regex::Regex;
use swc_core::common::Mark;
use swc_core::ecma::ast::{CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, Str};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::is_ident_undefined;
use crate::compiler::Context;
use crate::config::Platform;

const MAKO_REQUIRE: &str = "__mako_require__";

pub struct MakoRequire {
    pub unresolved_mark: Mark,
    pub ignores: Vec<Regex>,
    pub context: Arc<Context>,
}

impl MakoRequire {
    fn replace_require(&mut self, ident: &mut Ident) {
        // replace native require to __mako_require__ except ignored identities
        if is_ident_undefined(ident, "require", &self.unresolved_mark) {
            *ident = Ident::new(MAKO_REQUIRE.into(), ident.span, ident.ctxt);
        }
    }
}

impl VisitMut for MakoRequire {
    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        if let (
            Some(ExprOrSpread {
                expr: box Expr::Lit(Lit::Str(Str { value, .. })),
                ..
            }),
            Callee::Expr(box Expr::Ident(ident)),
        ) = (&call_expr.args.first(), &mut call_expr.callee)
        {
            let src = value.to_string();
            // replace native require call expression to __mako_require__ except ignored identities
            if !self.ignores.iter().any(|i| i.is_match(&src)) {
                self.replace_require(ident);
            }
            // skip visit callee to avoid replace ignored require by self.visit_mut_ident
            call_expr.args.visit_mut_children_with(self);
        } else {
            call_expr.visit_mut_children_with(self);
        }
    }

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if self.context.config.experimental.ignore_non_literal_require
            && let Platform::Node = self.context.config.platform
        {
            return;
        }
        self.replace_require(ident);
    }
}

#[cfg(test)]
mod tests {
    use regex::Regex;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::MakoRequire;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_require_ident() {
        assert_eq!(run(r#"require"#, vec![]), r#"__mako_require__;"#);
    }

    #[test]
    fn test_require_ident_in_args() {
        assert_eq!(
            run(r#"foo("", require)"#, vec![]),
            r#"foo("", __mako_require__);"#
        );
    }

    #[test]
    fn test_require_fn() {
        assert_eq!(
            run(r#"require("foo")"#, vec![]),
            r#"__mako_require__("foo");"#
        );
    }

    #[test]
    fn test_require_is_defined() {
        assert_eq!(
            run(r#"const require = 1;require("foo")"#, vec![]),
            r#"
const require = 1;
require("foo");
            "#
            .trim()
        );
    }

    #[test]
    fn test_with_ignores() {
        assert_eq!(
            run(r#"require("foo")"#, vec![Regex::new("foo").unwrap()]),
            r#"require("foo");"#
        );
    }

    fn run(js_code: &str, ignores: Vec<Regex>) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = MakoRequire {
                ignores,
                unresolved_mark: ast.unresolved_mark,
                context: test_utils.context.clone(),
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
