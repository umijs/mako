use mako_core::swc_ecma_ast::{Expr, ExprOrSpread, Lit};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use super::utils::{arrow_fn, call, id, member_call, member_prop, promise_resolve};
use crate::plugins::javascript::is_dynamic_import;

pub struct DynamicImportToRequire {}

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
                    // Promise.resolve().then(() => require('xxx'))
                    *expr = member_call(
                        promise_resolve(),
                        member_prop("then"),
                        vec![ExprOrSpread {
                            spread: None,
                            expr: Box::new(arrow_fn(
                                vec![],
                                call(
                                    Expr::Ident(id("require")),
                                    vec![ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Lit(Lit::Str(
                                            source.value.clone().into(),
                                        ))),
                                    }],
                                ),
                            )),
                        }],
                    );
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
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
        let context = std::sync::Arc::new(Default::default());
        let mut visitor = super::DynamicImportToRequire {};
        crate::transformers::test_helper::transform_js_code(code, &mut visitor, &context)
    }
}
