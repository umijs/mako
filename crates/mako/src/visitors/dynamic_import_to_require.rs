use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{Expr, ExprOrSpread, Lit};
use mako_core::swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast_2::utils::is_dynamic_import;

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
    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_visit::VisitMutWith;

    use super::DynamicImportToRequire;
    use crate::ast_2::tests::TestUtils;

    #[test]
    fn test_basic_import() {
        assert_eq!(
            run(r#"const testModule = import('test-module');"#,),
            r#"const testModule = Promise.resolve().then(()=>require("test-module"));"#.trim()
        );
    }

    #[test]
    fn test_chained_import() {
        assert_eq!(
            run(r#"import('test-module').then(() => (import('test-module-2')));"#,),
            r#"Promise.resolve().then(()=>require("test-module")).then(()=>(Promise.resolve().then(()=>require("test-module-2"))));"#.trim()
        );
        assert_eq!(
            run(r#"
Promise.all([
    import('test-1'),
    import('test-2'),
    import('test-3'),
]).then(() => {});
            "#,),
            r#"
Promise.all([
    Promise.resolve().then(()=>require("test-1")),
    Promise.resolve().then(()=>require("test-2")),
    Promise.resolve().then(()=>require("test-3"))
]).then(()=>{});
            "#
            .trim()
        );
    }

    #[test]
    fn test_import_with_comment() {
        assert_eq!(
            run(r#"
import(/* test comment */ 'my-module');
import('my-module' /* test comment */ );
            "#,),
            r#"
Promise.resolve().then(()=>require("my-module"));
Promise.resolve().then(()=>require("my-module"));
            "#
            .trim()
        );
    }

    // TODO: support this?
    #[test]
    #[ignore]
    fn test_dynamic_import() {
        crate::assert_display_snapshot!(run(r#"
import(MODULE);
let i = 0;
import(i++);
import(fn());
async () => import(await "x");
function* f() { import(yield "x"); }
            "#,));
    }

    // TODO: support this?
    #[test]
    #[ignore]
    fn test_template_argument() {
        crate::assert_display_snapshot!(run(r#"
import(`1`);
import(tag`2`);
import(`3-${MODULE}`);
            "#,));
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code.to_string());
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DynamicImportToRequire {
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
