use std::sync::Arc;

use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::{
    AssignExpr, BlockStmt, CallExpr, Decl, Expr, ExprOrSpread, ExprStmt, Stmt, TryStmt,
};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::{Dependency, ResolveType};
use crate::plugins::javascript::{get_first_arg_str, is_commonjs_require};
use crate::resolve;
use crate::transformers::transform_dep_replacer::miss_throw_stmt;

pub struct TryResolve<'a> {
    pub path: String,
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
}

impl TryResolve<'_> {
    pub fn handle_call_expr(&mut self, call_expr: &mut CallExpr) {
        if is_commonjs_require(call_expr, &self.unresolved_mark) {
            let first_arg = get_first_arg_str(call_expr);
            if let Some(source) = first_arg {
                let result = resolve::resolve(
                    self.path.as_str(),
                    &Dependency {
                        source: source.clone(),
                        resolve_as: None,
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
                    // e.g. var x = require('x');
                    Stmt::Decl(Decl::Var(var_decl)) => {
                        for decl in &mut var_decl.decls {
                            if let Some(Expr::Call(call_expr)) = decl.init.as_deref_mut() {
                                self.handle_call_expr(call_expr);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        stmt.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_transforms::resolver;
    use mako_core::swc_ecma_visit::VisitMutWith;

    use crate::ast::build_js_ast;

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
        let context: std::sync::Arc<crate::compiler::Context> =
            std::sync::Arc::new(Default::default());
        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("test.js", code, &context).unwrap();
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            let mut visitor = super::TryResolve {
                path: "test.js".to_string(),
                context: &context,
                unresolved_mark: ast.unresolved_mark,
            };
            crate::test_helper::transform_ast_with(
                &mut ast.ast,
                &mut visitor,
                &context.meta.script.cm,
            )
        })
    }
}
