use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{CallExpr, Expr, ExprOrSpread, ExprStmt, Lit, ModuleItem, Stmt, Str};
use swc_core::ecma::utils::{member_expr, ExprFactory};
use swc_core::ecma::visit::VisitMut;

use crate::ast::DUMMY_CTXT;

// TODO: add testcases
pub struct OptimizeDefineUtils {
    pub top_level_mark: Mark,
    pub unresolved_mark: Mark,
}
impl VisitMut for OptimizeDefineUtils {
    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        let mut no_directive_index = 0;
        for (index, item) in items.iter().enumerate() {
            if let Some(stmt) = item.as_stmt()
                && is_stmt_directive(stmt)
            {
                no_directive_index = index + 1
            } else {
                break;
            }
        }
        let mut iter = items.iter_mut().skip(no_directive_index);

        if let Some(item) = iter.next()
            && let Some(call_expr) = as_call_expr(item)
        {
            // Object.defineProperty(exports, "__esModule", { value: true })
            // find this means it's an es module, so continue with the optimization
            if let Some(callee_expr) = call_expr.callee.as_expr()
                && call_expr.args.len() == 3
                && is_object_define(callee_expr)
                && is_export_arg(call_expr.args.first())
                && is_string_lit_arg_with_value(call_expr.args.get(1), "__esModule")
                && is_obj_lit_arg(call_expr.args.get(2))
            {
                call_expr.callee = member_expr!(
                    DUMMY_CTXT.apply_mark(self.unresolved_mark),
                    DUMMY_SP,
                    __mako_require__.d
                )
                .as_callee();
            } else {
                return;
            }
        } else {
            return;
        }

        // cjs will inject most 6 stmts, so we only need to check the next 5 stmts
        // ref https://github.com/swc-project/swc/blob/ce76159e98ed6cc32104c5da848fe18ebea833b5/crates/swc_ecma_transforms_module/src/common_js.rs#L96
        for i in iter.take(5) {
            if let Some(call_expr) = as_call_expr(i) {
                // Object.defineProperty(exports, "default", {...})
                if let Some(callee_expr) = call_expr.callee.as_expr()
                    && call_expr.args.len() == 3
                    && is_object_define(callee_expr)
                    && is_export_arg(call_expr.args.first())
                    && is_string_lit_arg(call_expr.args.get(1))
                    && is_obj_lit_arg(call_expr.args.get(2))
                {
                    call_expr.callee = member_expr!(
                        DUMMY_CTXT.apply_mark(self.unresolved_mark),
                        DUMMY_SP,
                        __mako_require__.d
                    )
                    .as_callee();
                    return;
                }

                // _export(exports,{...})
                if let Some(callee_expr) = call_expr.callee.as_expr()
                    && let Some(callee_ident) = callee_expr.as_ident()
                    && call_expr.args.len() == 2
                    && is_export_arg(call_expr.args.first())
                    && is_obj_lit_arg(call_expr.args.get(1))
                    && callee_ident.sym == "_export"
                    // is private ident
                    && !callee_ident
                        .ctxt
                        .outer()
                        .is_descendant_of(self.top_level_mark)
                {
                    call_expr.callee = member_expr!(
                        DUMMY_CTXT.apply_mark(self.unresolved_mark),
                        DUMMY_SP,
                        __mako_require__.e
                    )
                    .as_callee();
                    return;
                }
            }
        }
    }
}

fn as_call_expr(item: &mut ModuleItem) -> Option<&mut CallExpr> {
    item.as_mut_stmt()
        .and_then(|stmt| stmt.as_mut_expr())
        .and_then(|expr| expr.expr.as_mut_call())
}

fn is_object_define(expr: &Expr) -> bool {
    expr.as_member()
        .map(|member| {
            let is_object = member
                .obj
                .as_ident()
                .map(|ident| ident.sym == "Object")
                .unwrap_or(false);

            is_object
                && member
                    .prop
                    .as_ident()
                    .map(|ident| ident.sym == "defineProperty")
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn is_export_arg(arg: Option<&ExprOrSpread>) -> bool {
    arg.map(|arg| {
        arg.spread.is_none()
            && arg
                .expr
                .as_ident()
                .map(|ident| ident.sym == "exports")
                .unwrap_or(false)
    })
    .unwrap_or(false)
}

fn is_string_lit_arg(arg: Option<&ExprOrSpread>) -> bool {
    arg.map(|arg| {
        arg.spread.is_none()
            && arg
                .expr
                .as_lit()
                .map(|lit| matches!(lit, Lit::Str(_)))
                .unwrap_or(false)
    })
    .unwrap_or(false)
}

fn is_string_lit_arg_with_value(arg: Option<&ExprOrSpread>, value: &str) -> bool {
    arg.map(|arg| {
        arg.spread.is_none()
            && arg
                .expr
                .as_lit()
                .map(|lit| {
                    if let Lit::Str(str_lit) = lit {
                        str_lit.value == value
                    } else {
                        false
                    }
                })
                .unwrap_or(false)
    })
    .unwrap_or(false)
}

fn is_obj_lit_arg(arg: Option<&ExprOrSpread>) -> bool {
    arg.map(|arg| arg.spread.is_none() && arg.expr.as_object().map(|_| true).unwrap_or(false))
        .unwrap_or(false)
}

fn is_stmt_directive(stmt: &Stmt) -> bool {
    if let Stmt::Expr(ExprStmt {
        expr: box Expr::Lit(Lit::Str(Str { value, .. })),
        ..
    }) = stmt
    {
        value.starts_with("use ")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_is_stmt_directive() {
        let tu = TestUtils::gen_js_ast(
            r#"
        "use strict";
        let a= 1;
        "#,
        );
        let ast = tu.ast.js();
        let use_strict_stmt = ast.ast.body[0].as_stmt().unwrap();

        assert!(is_stmt_directive(use_strict_stmt));
    }
}
