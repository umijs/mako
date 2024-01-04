use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{CallExpr, Expr, ExprOrSpread, Lit, ModuleItem};
use swc_core::ecma::utils::member_expr;
use swc_core::ecma::visit::VisitMut;

pub struct OptimizeDefineUtils {
    pub top_level_mark: Mark,
    pub unresolved_mark: Mark,
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

impl VisitMut for OptimizeDefineUtils {
    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        if let Some(item) = items.get_mut(0)
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
        for i in items.iter_mut().skip(1).take(5) {
            if let Some(call_expr) = as_call_expr(i) {
                if let Some(callee_expr) = call_expr.callee.as_expr()
                    && call_expr.args.len() == 3
                    && is_object_define(callee_expr)
                    && is_export_arg(call_expr.args.first())
                    && is_string_lit_arg(call_expr.args.get(1))
                    && is_obj_lit_arg(call_expr.args.get(2))
                {
                    call_expr.callee =
                        member_expr!(DUMMY_SP.apply_mark(self.unresolved_mark), require.d).into();

                    continue;
                }

                if let Some(callee_expr) = call_expr.callee.as_expr()
                    && let Some(callee_ident) = callee_expr.as_ident()
                    && call_expr.args.len() == 2
                    && is_export_arg(call_expr.args.first())
                    && is_obj_lit_arg(call_expr.args.get(1))
                    && callee_ident.sym.to_string() == "_export"
                    && !callee_ident
                        .span
                        .ctxt
                        .outer()
                        .is_descendant_of(self.top_level_mark)
                {
                    call_expr.callee =
                        member_expr!(DUMMY_SP.apply_mark(self.unresolved_mark), require.e).into()
                }
            }
        }
    }
}
