use std::sync::Arc;

use swc_core::common::Mark;
use swc_core::ecma::ast::{
    AssignExpr, BlockStmt, CallExpr, Decl, Expr, ExprOrSpread, ExprStmt, ReturnStmt, Stmt, TryStmt,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::{get_first_str_arg, is_commonjs_require};
use crate::compiler::Context;
use crate::module::{Dependency, ResolveType};
use crate::resolve;
use crate::visitors::dep_replacer::miss_throw_stmt;

pub struct TryResolve {
    pub path: String,
    pub context: Arc<Context>,
    pub unresolved_mark: Mark,
}

impl TryResolve {
    pub fn handle_call_expr(&mut self, call_expr: &mut CallExpr) {
        if is_commonjs_require(call_expr, &self.unresolved_mark) {
            let first_arg = get_first_str_arg(call_expr);
            if let Some(source) = first_arg {
                // support ignores config
                let mut deps = vec![Dependency {
                    source: source.clone(),
                    resolve_as: None,
                    resolve_type: ResolveType::Require,
                    order: 0,
                    span: None,
                }];
                self.context
                    .plugin_driver
                    .before_resolve(&mut deps, &self.context)
                    .unwrap(); // before_resolve won't panic
                if deps.is_empty() {
                    return;
                }
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
                    &self.context,
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

impl VisitMut for TryResolve {
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Try(box TryStmt {
            block: BlockStmt { stmts, .. },
            ..
        }) = stmt
        {
            for stmt in stmts {
                match stmt {
                    // e.g. return require('x');
                    Stmt::Return(ReturnStmt {
                        arg: Some(box Expr::Call(call_expr)),
                        ..
                    }) => {
                        self.handle_call_expr(call_expr);
                    }
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
                            // support `const a = 1 && require('not-found-module');`
                            // when decl.init is BinaryExpression, handle left and right recursively
                            // ref: https://github.com/umijs/mako/issues/1007
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
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::TryResolve;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_try_require() {
        assert_eq!(
            run(r#"try{require('foo')}catch(e){}"#),
            r#"
try {
    require(Object(function makoMissingModule() {
        var e = new Error("Cannot find module 'foo'");
        e.code = "MODULE_NOT_FOUND";
        throw e;
    }()));
} catch (e) {}
        "#
            .trim()
        );
    }

    #[test]
    fn test_try_require_support_return_stmt() {
        assert_eq!(
            run(r#"function a() {try{return require('foo')}catch(e){}}"#),
            r#"
function a() {
    try {
        return require(Object(function makoMissingModule() {
            var e = new Error("Cannot find module 'foo'");
            e.code = "MODULE_NOT_FOUND";
            throw e;
        }()));
    } catch (e) {}
}
        "#
            .trim()
        );
    }

    #[test]
    fn test_try_require_support_var_decl() {
        assert_eq!(
            run(r#"try{const x = require('foo')}catch(e){}"#),
            r#"
try {
    const x = require(Object(function makoMissingModule() {
        var e = new Error("Cannot find module 'foo'");
        e.code = "MODULE_NOT_FOUND";
        throw e;
    }()));
} catch (e) {}
        "#
            .trim()
        );
    }

    #[test]
    fn test_try_require_with_exports() {
        assert_eq!(
            run(r#"try{exports.xxx = require('foo');}catch(e){}"#),
            r#"
try {
    exports.xxx = require(Object(function makoMissingModule() {
        var e = new Error("Cannot find module 'foo'");
        e.code = "MODULE_NOT_FOUND";
        throw e;
    }()));
} catch (e) {}
        "#
            .trim()
        );
    }

    #[test]
    fn test_try_require_dont_support_import() {
        assert_eq!(
            run(r#"try{import('foo');}catch(e){}"#),
            r#"
try {
    import('foo');
} catch (e) {}
        "#
            .trim()
        );
    }

    #[test]
    fn test_try_require_handle_require_defined() {
        assert_eq!(
            run(r#"try{const require=1;require('foo');}catch(e){}"#),
            r#"
try {
    const require = 1;
    require('foo');
} catch (e) {}
        "#
            .trim()
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = TryResolve {
                path: "/path/should/not/exists.js".to_string(),
                context: test_utils.context.clone(),
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
