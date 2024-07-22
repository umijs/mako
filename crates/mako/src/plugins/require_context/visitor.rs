use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use swc_core::common::{Mark, Spanned};
use swc_core::ecma::ast::{CallExpr, Callee, Expr, ExprOrSpread};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use super::param::{ContextParam, ContextParamBuilder};
use crate::ast::error::{code_frame, ErrorSpan};
use crate::ast::utils::is_ident_undefined;
use crate::build::parse::ParseError;
use crate::compiler::Context;

pub struct RequireContextVisitor {
    pub(crate) current_path: PathBuf,
    pub(crate) unresolved_mark: Mark,
    pub(crate) context: Arc<Context>,
    pub(crate) res: Result<()>,
}

impl RequireContextVisitor {
    // is require.context(...)
    fn is_require_context(&self, call: &CallExpr) -> bool {
        match &call.callee {
            Callee::Expr(box Expr::Member(member_expr)) => {
                member_expr.obj.as_ident().map_or(false, |ident| {
                    is_ident_undefined(ident, "require", &self.unresolved_mark)
                }) && member_expr
                    .prop
                    .as_ident()
                    .map_or(false, |ident| ident.sym.eq("context"))
            }

            _ => false,
        }
    }

    fn to_context_param(&self, call: &CallExpr) -> Option<ContextParam> {
        let builder = ContextParamBuilder::default()
            .relative_path(call.args.first())
            .sub_directories(call.args.get(1))
            .reg_expr(call.args.get(2))
            .mode(call.args.get(3));

        builder.build()
    }

    fn is_valid_args(&self, args: &Vec<ExprOrSpread>) -> bool {
        if !args.is_empty() && args.len() <= 4 {
            args.iter().all(|arg| {
                if arg.spread.is_some() {
                    return false;
                }

                arg.expr.as_lit().is_some()
            })
        } else {
            false
        }
    }
}

impl VisitMut for RequireContextVisitor {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(call_expr)
                if self.is_require_context(call_expr) && self.is_valid_args(&call_expr.args) =>
            {
                if let Some(context_param) = self.to_context_param(call_expr) {
                    if let Ok(context_module_id) =
                        context_param.to_context_id(&self.current_path, &self.context)
                    {
                        let call_expr = quote_ident!("__mako_require__")
                            .as_call(expr.span(), vec![quote_str!(context_module_id).as_arg()]);

                        *expr = call_expr;
                    } else {
                        self.res = Err(anyhow!(ParseError::InvalidExpression {
                            path: self.current_path.to_string_lossy().to_string(),
                            message: code_frame(
                                ErrorSpan::Js(call_expr.span()),
                                "Bad context path",
                                self.context.clone(),
                            )
                        }));
                    }
                    return;
                }
            }
            _ => {}
        };

        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use percent_encoding::percent_decode_str;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::*;
    use crate::ast::tests::TestUtils;

    fn transform_code(code: &str) -> String {
        let mut tu = TestUtils::gen_js_ast(code);

        let js_ast = tu.ast.js_mut();

        GLOBALS.set(&tu.context.meta.script.globals, || {
            js_ast.ast.visit_mut_with(&mut RequireContextVisitor {
                current_path: PathBuf::from("/project/src/index.js"),
                unresolved_mark: js_ast.unresolved_mark,
                context: tu.context.clone(),
                res: Ok(()),
            });
        });

        percent_decode_str(&tu.js_ast_to_code())
            .decode_utf8()
            .unwrap()
            .to_string()
    }

    #[test]
    fn normal_sync() {
        assert_eq!(
            transform_code(r#" const ctxt = require.context("./", false, /\.js$/, "sync"); "#),
            r#"
            const ctxt = __mako_require__("virtual:context?root=/project/src&sub=false&reg=\.js$&mode=sync&ig=false");
            "#
                .trim()
        );
    }

    #[test]
    fn normal_sync_with_sub_directories() {
        assert_eq!(
            transform_code(r#" const ctxt = require.context("./", true, /\.js$/, "sync"); "#),
            r#"
            const ctxt = __mako_require__("virtual:context?root=/project/src&sub=true&reg=\.js$&mode=sync&ig=false");
            "#
                .trim()
        )
    }

    #[test]
    fn normal_sync_ignore_case_sensitive() {
        assert_eq!(
            transform_code(r#"
               const ctxt = require.context("./", false, /\.js$/i, "sync");
            "#)
            ,
            r#"
            const ctxt = __mako_require__("virtual:context?root=/project/src&sub=false&reg=\.js$&mode=sync&ig=true");
            "#
                .trim()
        );
    }

    #[test]
    fn all_default_value() {
        assert_eq!(
            transform_code(r#"
               const ctxt = require.context("./");
            "#)
            ,
            r#"
            const ctxt = __mako_require__("virtual:context?root=/project/src&sub=true&reg=^\.\/.*$&mode=sync&ig=false"); 
           "#
                .trim()
        );
    }

    #[ignore = "later"]
    #[test]
    fn invalid_require_context() {
        assert_eq!(
            transform_code(r#" const ctxt = require.context("./", foo, /\.js$/i, "sync"); "#,),
            r#"
        const ctxt = require.context("./", foo, /\.js$/i, "sync");
        "#
            .trim()
        );
    }
}
