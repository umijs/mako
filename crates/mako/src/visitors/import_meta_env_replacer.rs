use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{
    Expr, Ident, KeyValueProp, MemberExpr, MemberProp, MetaPropExpr, MetaPropKind, ObjectLit, Prop,
    PropOrSpread,
};
use swc_core::ecma::atoms::js_word;
use swc_core::ecma::utils::{quote_ident, quote_str};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

#[derive(Debug)]
pub(crate) struct ImportMetaEnvReplacer {
    pub(crate) mode: String,
}

impl ImportMetaEnvReplacer {
    pub(crate) fn new(mode: String) -> Self {
        Self { mode }
    }
}

impl VisitMut for ImportMetaEnvReplacer {
    fn visit_mut_member_expr(&mut self, member_expr: &mut MemberExpr) {
        match member_expr {
            MemberExpr {
                obj:
                    box Expr::Member(MemberExpr {
                        obj:
                            box Expr::MetaProp(MetaPropExpr {
                                kind: MetaPropKind::ImportMeta,
                                ..
                            }),
                        prop:
                            MemberProp::Ident(Ident {
                                sym: js_word!("env"),
                                ..
                            }),
                        ..
                    }),
                ..
            } => {
                // replace import.meta.env.MODE with "({ MODE: 'production' }).MODE"
                *member_expr.obj = Expr::Paren(swc_core::ecma::ast::ParenExpr {
                    span: DUMMY_SP,
                    expr: ObjectLit {
                        props: vec![PropOrSpread::Prop(
                            Prop::KeyValue(KeyValueProp {
                                key: quote_ident!("MODE").into(),
                                value: quote_str!(self.mode.clone()).into(),
                            })
                            .into(),
                        )],
                        span: DUMMY_SP,
                    }
                    .into(),
                });
            }

            _ => member_expr.visit_mut_children_with(self),
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Member(MemberExpr {
                obj:
                    box Expr::MetaProp(MetaPropExpr {
                        kind: MetaPropKind::ImportMeta,
                        ..
                    }),
                prop:
                    MemberProp::Ident(Ident {
                        sym: js_word!("env"),
                        ..
                    }),
                ..
            }) => {
                // replace import.meta.env with "({ MODE: 'production' })"
                *expr = Expr::Object(ObjectLit {
                    props: vec![PropOrSpread::Prop(
                        Prop::KeyValue(KeyValueProp {
                            key: quote_ident!("MODE").into(),
                            value: quote_str!(self.mode.clone()).into(),
                        })
                        .into(),
                    )],
                    span: DUMMY_SP,
                });
            }
            _ => expr.visit_mut_children_with(self),
        }
    }
}

#[cfg(test)]
mod tests {

    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::ImportMetaEnvReplacer;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_import_meta_env() {
        assert_eq!(
            run(
                r#"typeof import.meta.env === "object" ? import.meta.env.MODE : process.env.NODE_ENV"#
            ),
            r#"typeof {
    MODE: "development"
} === "object" ? ({
    MODE: "development"
}).MODE : process.env.NODE_ENV;"#
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = ImportMetaEnvReplacer::new("development".to_string());
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
