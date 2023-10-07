use std::sync::Arc;

use swc_ecma_ast::{BlockStmt, Expr, ExprOrSpread, ExprStmt, Stmt, TryStmt};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::{get_first_arg_str, is_commonjs_require};
use crate::compiler::Context;
use crate::module::{Dependency, ResolveType};
use crate::resolve::{self, Resolvers};
use crate::transformers::transform_dep_replacer::miss_throw_stmt;

pub struct TryResolve<'a> {
    pub path: String,
    pub resolvers: &'a Resolvers,
    pub context: &'a Arc<Context>,
}

impl VisitMut for TryResolve<'_> {
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Try(box TryStmt {
            block: BlockStmt { stmts, .. },
            ..
        }) = stmt
        {
            for stmt in stmts {
                if let Stmt::Expr(ExprStmt {
                    expr: box Expr::Call(call_expr),
                    ..
                }) = stmt
                {
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
                                self.resolvers,
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
        }
        stmt.visit_mut_children_with(self);
    }
}
