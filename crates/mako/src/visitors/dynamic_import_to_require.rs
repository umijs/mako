use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{Expr, ExprOrSpread, Ident, Lit, MemberExpr, Stmt, VarDeclKind};
use swc_core::ecma::utils::{
    member_expr, private_ident, quote_ident, quote_str, ExprFactory, IsDirective,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::is_dynamic_import;
pub struct DynamicImportToRequire {
    pub unresolved_mark: Mark,
    changed: bool,
    interop: Ident,
}

impl DynamicImportToRequire {
    pub fn new(unresolved_mark: Mark) -> Self {
        let interop = private_ident!("interop");
        Self {
            unresolved_mark,
            changed: false,
            interop,
        }
    }
}
// import('xxx') -> Promise.resolve().then(() => require('xxx'))
impl VisitMut for DynamicImportToRequire {
    fn visit_mut_module(&mut self, n: &mut swc_core::ecma::ast::Module) {
        n.visit_mut_children_with(self);

        if self.changed {
            let insert_at = n
                .body
                .iter()
                .position(|module_item| {
                    !module_item
                        .as_stmt()
                        .map_or(false, |stmt| stmt.is_directive())
                })
                .unwrap();
            let require_interop = quote_ident!("__mako_require__").as_call(
                DUMMY_SP,
                vec![quote_str!("@swc/helpers/_/_interop_require_wildcard").as_arg()],
            );

            let stmt: Stmt = Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: require_interop.into(),
                prop: quote_ident!("_").into(),
            })
            .into_var_decl(VarDeclKind::Var, self.interop.clone().into())
            .into();
            n.body.insert(insert_at, stmt.into());
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_dynamic_import(call_expr) {
                self.changed = true;
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    let source_require: Expr =
                        quote_ident!(DUMMY_SP.apply_mark(self.unresolved_mark), "require")
                            .as_call(DUMMY_SP, vec![quote_str!(source.value.clone()).as_arg()]);
                    // Promise.resolve()
                    let promise_resolve: Box<Expr> = member_expr!(DUMMY_SP, Promise.resolve)
                        .as_call(
                            DUMMY_SP,
                            vec![ExprOrSpread {
                                spread: None,
                                expr: Box::new(source_require),
                            }],
                        )
                        .into();

                    let interop_call = quote_ident!(DUMMY_SP, self.interop.as_ref());
                    let promised_lazy_require: Expr =
                        member_expr!(@EXT,DUMMY_SP, promise_resolve, then)
                            .as_call(DUMMY_SP, vec![interop_call.as_arg()]);

                    *expr = promised_lazy_require;
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::DynamicImportToRequire;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_basic_import() {
        assert_eq!(
            run(r#"const testModule = import('test-module');"#,),
            r#"var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;
const testModule = Promise.resolve(require("test-module")).then(interop);"#
                .trim()
        );
    }

    #[test]
    fn test_chained_import() {
        assert_eq!(
            run(r#"import('test-module').then(() => (import('test-module-2')));"#,),
            r#"var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;
Promise.resolve(require("test-module")).then(interop).then(()=>(Promise.resolve(require("test-module-2")).then(interop)));"#.trim()
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
var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;
Promise.all([
    Promise.resolve(require("test-1")).then(interop),
    Promise.resolve(require("test-2")).then(interop),
    Promise.resolve(require("test-3")).then(interop)
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
var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;
Promise.resolve(require("my-module")).then(interop);
Promise.resolve(require("my-module")).then(interop);
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
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DynamicImportToRequire::new(ast.unresolved_mark);
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
