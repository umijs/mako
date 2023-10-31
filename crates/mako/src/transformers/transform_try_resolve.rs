use std::sync::Arc;

use mako_core::swc_ecma_ast::{
    AssignExpr, BlockStmt, CallExpr, Expr, ExprOrSpread, ExprStmt, Stmt, TryStmt,
};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::{get_first_arg_str, is_commonjs_require};
use crate::compiler::Context;
use crate::module::{Dependency, ResolveType};
use crate::resolve;
use crate::transformers::transform_dep_replacer::miss_throw_stmt;

pub struct TryResolve<'a> {
    pub path: String,
    pub context: &'a Arc<Context>,
}

impl TryResolve<'_> {
    pub fn handle_call_expr(&mut self, call_expr: &mut CallExpr) {
        if is_commonjs_require(call_expr, None) {
            let first_arg = get_first_arg_str(call_expr);
            if let Some(source) = first_arg {
                let result = resolve::resolve(
                    self.path.as_str(),
                    &Dependency {
                        source: source.clone(),
                        resolve_type: ResolveType::Require,
                        order: 0,
                        span: None,
                    },
                    &self.context.resolvers,
                    self.context,
                );
                if result.is_err() {
                    call_expr.args[0] = ExprOrSpread {
                        spread: None,
                        expr: Box::new(miss_throw_stmt(&source)),
                    };
                }
            }
        }
    }
}

impl VisitMut for TryResolve<'_> {
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Try(box TryStmt {
            block: BlockStmt { stmts, .. },
            ..
        }) = stmt
        {
            for stmt in stmts {
                match stmt {
                    Stmt::Expr(ExprStmt {
                        expr: box Expr::Call(call_expr),
                        ..
                    })
                    | Stmt::Expr(ExprStmt {
                        expr:
                            box Expr::Assign(AssignExpr {
                                right: box Expr::Call(call_expr),
                                ..
                            }),
                        ..
                    }) => self.handle_call_expr(call_expr),
                    _ => {}
                }
            }
        }
        stmt.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_try_require() {
        crate::assert_display_snapshot!(transform(
            r#"
            try {
                require('foo');
            } catch (e) {
                console.log(e);
            }
            "#,
        ));
    }

    #[test]
    fn test_try_require_with_exports() {
        crate::assert_display_snapshot!(transform(
            r#"
            try {
                exports.xxx = require('foo');
            } catch (e) {
                console.log(e);
            }
            "#,
        ));
    }

    #[test]
    fn test_try_import_do_not_resolve() {
        crate::assert_display_snapshot!(transform(
            r#"
            try {
                import('foo');
            } catch (e) {
                console.log(e);
            }
            "#,
        ));
    }

    fn transform(code: &str) -> String {
        let context = std::sync::Arc::new(Default::default());
        let mut visitor = super::TryResolve {
            path: "test.js".to_string(),
            context: &context,
        };
        crate::transformers::test_helper::transform_js_code(code, &mut visitor, &context)
    }
}
