use swc_core::common::Mark;
use swc_core::ecma::ast::{Expr, Lit, NewExpr, Str};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::{is_ident_undefined, is_remote_or_data};

pub struct WorkerModule {
    unresolved_mark: Mark,
}

impl WorkerModule {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self { unresolved_mark }
    }
}

impl VisitMut for WorkerModule {
    fn visit_mut_new_expr(&mut self, new_expr: &mut NewExpr) {
        if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty())
            || !new_expr.callee.is_ident()
        {
            return;
        }

        if let box Expr::Ident(ident) = &mut new_expr.callee {
            #[allow(clippy::needless_borrow)]
            if is_ident_undefined(&ident, "Worker", &self.unresolved_mark) {
                let args = new_expr.args.as_mut().unwrap();

                // new Worker(new URL(''), base);
                if let Expr::New(new_expr) = &mut *args[0].expr {
                    if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty())
                        || !new_expr.callee.is_ident()
                    {
                        return;
                    }

                    if let Some(ident) = &new_expr.callee.as_ident() {
                        if is_ident_undefined(ident, "URL", &self.unresolved_mark) {
                            // new URL('');
                            let args = new_expr.args.as_mut().unwrap();
                            if let Some(Lit::Str(ref mut str)) = &mut args[0].expr.as_mut_lit() {
                                if !is_remote_or_data(&str.value) {
                                    self.replace_source(str);
                                }
                            }
                        }
                    }
                }
            }
        }

        new_expr.visit_mut_children_with(self);
    }
}

impl WorkerModule {
    fn replace_source(&mut self, source: &mut Str) {
        /* A source file can be a async module and a worker entry at the same time,
         * we need to add a worker query to distinguish worker from async module, or else
         * those two chunks will use the same id, bundled dist will be broken.
         */
        let to_replace = format!("{}?asworker", &source.value.to_string());
        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}
