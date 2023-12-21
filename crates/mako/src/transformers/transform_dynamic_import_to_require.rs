use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{Expr, ExprOrSpread, Lit};
use mako_core::swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::plugins::javascript::is_dynamic_import;

pub struct DynamicImportToRequire {
    pub unresolved_mark: Mark,
}

// import('xxx') -> Promise.resolve().then(() => require('xxx'))
impl VisitMut for DynamicImportToRequire {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_dynamic_import(call_expr) {
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    // Promise.resolve()
                    let promise_resolve: Box<Expr> = member_expr!(DUMMY_SP, Promise.resolve)
                        .as_call(DUMMY_SP, vec![])
                        .into();

                    // () => require( source.value... )
                    let lazy_require =
                        quote_ident!(DUMMY_SP.apply_mark(self.unresolved_mark), "require")
                            .as_call(DUMMY_SP, vec![quote_str!(source.value.clone()).as_arg()])
                            .into_lazy_arrow(vec![]);

                    // Promise.resolve().then(() => require("xxx"))
                    let promised_lazy_require: Expr =
                        member_expr!(@EXT,DUMMY_SP, promise_resolve, then)
                            .as_call(DUMMY_SP, vec![lazy_require.as_arg()]);

                    *expr = promised_lazy_require;
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::swc_common::{Mark, GLOBALS};

    use crate::compiler::Context;

    #[test]
    fn test_basic_import() {
        crate::assert_display_snapshot!(transform(
            r#"
const testModule = import('test-module');
            "#,
        ));
    }

    #[test]
    fn test_chained_import() {
        crate::assert_display_snapshot!(transform(
            r#"
import('test-module').then(() => (
import('test-module-2')
));

Promise.all([
import('test-1'),
import('test-2'),
import('test-3'),
]).then(() => {});
            "#,
        ));
    }

    // TODO: support this?
    #[test]
    fn test_dynamic_import() {
        crate::assert_display_snapshot!(transform(
            r#"
import(MODULE);

let i = 0;
import(i++);

import(fn());

async () => import(await "x");

function* f() { import(yield "x"); }
            "#,
        ));
    }

    #[test]
    fn test_import_with_comment() {
        crate::assert_display_snapshot!(transform(
            r#"
import(/* test comment */ 'my-module');
import('my-module' /* test comment */ );
            "#,
        ));
    }

    // TODO: support this?
    #[test]
    fn test_template_argument() {
        crate::assert_display_snapshot!(transform(
            r#"
import(`1`);
import(tag`2`);
import(`3-${MODULE}`);
            "#,
        ));
    }

    fn transform(code: &str) -> String {
        let context: Arc<Context> = Arc::new(Default::default());
        GLOBALS.set(&context.meta.script.globals, || {
            let mut visitor = super::DynamicImportToRequire {
                unresolved_mark: Mark::new(),
            };
            crate::transformers::test_helper::transform_js_code(code, &mut visitor, &context)
        })
    }
}
