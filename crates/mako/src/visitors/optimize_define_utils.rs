use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{CallExpr, Expr, ExprOrSpread, Lit, ModuleItem, Stmt, Str};
use swc_core::ecma::utils::{member_expr, IsDirective};
use swc_core::ecma::visit::VisitMut;

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
                && is_directive_judged_by_stmt_value_and_raw(stmt.clone())
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
                call_expr.callee =
                    member_expr!(DUMMY_SP.apply_mark(self.unresolved_mark), require.d).into();
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
                    call_expr.callee =
                        member_expr!(DUMMY_SP.apply_mark(self.unresolved_mark), require.d).into();
                    return;
                }

                // _export(exports,{...})
                if let Some(callee_expr) = call_expr.callee.as_expr()
                    && let Some(callee_ident) = callee_expr.as_ident()
                    && call_expr.args.len() == 2
                    && is_export_arg(call_expr.args.first())
                    && is_obj_lit_arg(call_expr.args.get(1))
                    && callee_ident.sym.to_string() == "_export"
                    // is private ident
                    && !callee_ident
                        .span
                        .ctxt
                        .outer()
                        .is_descendant_of(self.top_level_mark)
                {
                    call_expr.callee =
                        member_expr!(DUMMY_SP.apply_mark(self.unresolved_mark), require.e).into();
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
                .map(|ident| ident.sym.to_string() == "Object")
                .unwrap_or(false);

            is_object
                && member
                    .prop
                    .as_ident()
                    .map(|ident| ident.sym.to_string() == "defineProperty")
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
                .map(|ident| ident.sym.to_string() == "exports")
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
                        str_lit.value.to_string() == value
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

fn is_directive_judged_by_stmt_value_and_raw(stmt: Stmt) -> bool {
    match stmt.as_ref() {
        Some(Stmt::Expr(expr)) => match &*expr.expr {
            Expr::Lit(Lit::Str(Str { raw: Some(raw), .. })) => {
                raw.starts_with("\"use ") || raw.starts_with("'use ")
            }
            Expr::Lit(Lit::Str(Str {
                value: v,
                raw: None,
                ..
            })) => v.to_string().starts_with("use "),
            _ => false,
        },
        _ => false,
    }
}
