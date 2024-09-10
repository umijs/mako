use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::Value;
use swc_core::common::{Mark, Span, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, Bool, ComputedPropName, Expr, ExprOrSpread, Ident, KeyValueProp, Lit, MemberExpr,
    MemberProp, ModuleItem, Null, Number, ObjectLit, Prop, PropOrSpread, Stmt, Str,
};
use swc_core::ecma::utils::{quote_ident, ExprExt};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::ConfigError;

#[derive(Debug)]
pub struct EnvReplacer {
    unresolved_mark: Mark,
    define: HashMap<String, Expr>,
}

impl EnvReplacer {
    pub fn new(define: HashMap<String, Expr>, unresolved_mark: Mark) -> Self {
        Self {
            unresolved_mark,
            define,
        }
    }

    fn get_define_env(&self, key: &str) -> Option<Expr> {
        self.define.get(key).cloned()
    }
}
impl VisitMut for EnvReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { span, .. }) = expr {
            // 先判断 env 中的变量名称，是否是上下文中已经存在的变量名称
            if span.ctxt.outer() != self.unresolved_mark {
                expr.visit_mut_children_with(self);
                return;
            }
        }

        match expr {
            Expr::Member(MemberExpr { obj, prop, .. }) => {
                let mut member_visit_path = match prop {
                    MemberProp::Ident(Ident { sym, .. }) => sym.to_string(),
                    MemberProp::Computed(ComputedPropName {
                        expr: expr_compute, ..
                    }) => match expr_compute.as_ref() {
                        Expr::Lit(Lit::Str(Str { value, .. })) => value.to_string(),

                        Expr::Lit(Lit::Num(Number { value, .. })) => value.to_string(),
                        _ => {
                            obj.visit_mut_with(self);
                            expr_compute.visit_mut_with(self);
                            return;
                        }
                    },
                    _ => return,
                };

                let mut current_member_obj = obj.as_mut();

                while let Expr::Member(MemberExpr { obj, prop, .. }) = current_member_obj {
                    match prop {
                        MemberProp::Ident(Ident { sym, .. }) => {
                            member_visit_path.push('.');
                            member_visit_path.push_str(sym.as_ref());
                        }
                        MemberProp::Computed(ComputedPropName {
                            expr: expr_compute, ..
                        }) => match expr_compute.as_ref() {
                            Expr::Lit(Lit::Str(Str { value, .. })) => {
                                member_visit_path.push('.');
                                member_visit_path.push_str(value.as_ref());
                            }

                            Expr::Lit(Lit::Num(Number { value, .. })) => {
                                member_visit_path.push('.');
                                member_visit_path.push_str(&value.to_string());
                            }
                            _ => {
                                obj.visit_mut_with(self);
                                expr_compute.visit_mut_with(self);
                                return;
                            }
                        },
                        _ => return,
                    }
                    current_member_obj = obj.as_mut();
                }

                if let Expr::Ident(Ident { sym, span, .. }) = current_member_obj {
                    if span.ctxt.outer() != self.unresolved_mark {
                        return;
                    }
                    member_visit_path.push('.');
                    member_visit_path.push_str(sym.as_ref());
                }

                let member_visit_path = member_visit_path
                    .split('.')
                    .rev()
                    .collect::<Vec<&str>>()
                    .join(".");

                if let Some(env) = self.get_define_env(&member_visit_path) {
                    *expr = env
                }
            }

            Expr::Ident(Ident { sym, .. }) => {
                if let Some(env) = self.get_define_env(sym.as_ref()) {
                    *expr = env
                }
            }
            _ => (),
        }

        expr.visit_mut_children_with(self);
    }
}

pub fn build_env_map(
    env_map: HashMap<String, Value>,
    context: &Arc<Context>,
) -> Result<HashMap<String, Expr>> {
    let mut map = HashMap::new();
    for (k, v) in env_map.into_iter() {
        let expr = get_env_expr(v, context)?;
        map.insert(k, expr);
    }
    Ok(map)
}

fn get_env_expr(v: Value, context: &Arc<Context>) -> Result<Expr> {
    match v {
        Value::String(v) => {
            let safe_value = if Value::from_str(&v).map_or(false, |t| t.is_object()) {
                format!("({})", v)
            } else {
                v.clone()
            };

            let module = {
                // the string content is treat as expression, so it has to be parsed
                let mut ast =
                    JsAst::build("_mako_internal/_define_.js", &safe_value, context.clone())
                        .unwrap();
                ast.ast.visit_mut_with(&mut strip_span());
                ast.ast.body.pop().unwrap()
            };

            match module {
                ModuleItem::Stmt(Stmt::Expr(stmt_expr)) => {
                    return Ok(stmt_expr.expr.as_expr().clone());
                }
                _ => Err(anyhow!(ConfigError::InvalidateDefineConfig(v))),
            }
        }
        Value::Bool(v) => Ok(Bool {
            span: DUMMY_SP,
            value: v,
        }
        .into()),
        Value::Number(v) => Ok(Number {
            span: DUMMY_SP,
            raw: None,
            value: v.as_f64().unwrap(),
        }
        .into()),
        Value::Array(val) => {
            let mut elems = vec![];
            for item in val.iter() {
                elems.push(Some(ExprOrSpread {
                    spread: None,
                    expr: get_env_expr(item.clone(), context)?.into(),
                }));
            }

            Ok(ArrayLit {
                span: DUMMY_SP,
                elems,
            }
            .into())
        }
        Value::Null => Ok(Null { span: DUMMY_SP }.into()),
        Value::Object(val) => {
            let mut props = vec![];
            for (key, value) in val.iter() {
                let prop = PropOrSpread::Prop(
                    Prop::KeyValue(KeyValueProp {
                        key: quote_ident!(key.clone()).into(),
                        value: get_env_expr(value.clone(), context)?.into(),
                    })
                    .into(),
                );
                props.push(prop);
            }
            Ok(ObjectLit {
                span: DUMMY_SP,
                props,
            }
            .into())
        }
    }
}

struct SpanStrip {}
impl VisitMut for SpanStrip {
    fn visit_mut_span(&mut self, span: &mut Span) {
        *span = DUMMY_SP;
    }
}

fn strip_span() -> impl VisitMut {
    SpanStrip {}
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use maplit::hashmap;
    use regex::Regex;
    use serde_json::{json, Value};
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::{build_env_map, EnvReplacer};
    use crate::ast::tests::TestUtils;
    use crate::compiler::Context;

    #[should_panic = "define value 'for(;;)console.log()' is not an Expression"]
    #[test]
    fn test_wrong_define_value() {
        let context: Arc<Context> = Arc::new(Default::default());
        build_env_map(
            hashmap! {
                "wrong".to_string() => json!("for(;;)console.log()")
            },
            &context,
        )
        .unwrap();
    }

    #[should_panic = "define value 'for(;;)console.log()' is not an Expression"]
    #[test]
    fn test_nested_wrong_define_value() {
        let context: Arc<Context> = Arc::new(Default::default());
        build_env_map(
            hashmap! {
                "parent".to_string() =>
                json!({"wrong": "for(;;)console.log()" })
            },
            &context,
        )
        .unwrap();
    }

    #[test]
    fn test_boolean() {
        assert_eq!(
            run(
                r#"log(A)"#,
                hashmap! {
                    "A".to_string() => json!(true)
                }
            ),
            "log(true);"
        );
    }

    #[test]
    fn test_number() {
        assert_eq!(
            run(
                r#"log(A)"#,
                hashmap! {
                    "A".to_string() => json!(1)
                }
            ),
            "log(1);"
        );
    }

    #[test]
    fn test_string() {
        assert_eq!(
            run(
                r#"log(A)"#,
                hashmap! {
                    "A".to_string() => json!("\"foo\"")
                }
            ),
            "log(\"foo\");"
        );
    }

    #[test]
    fn test_array() {
        assert_eq!(
            run(
                r#"log(A)"#,
                hashmap! {
                    "A".to_string() => json!([1,true,"\"foo\""])
                }
            ),
            "log([1,true,\"foo\"]);".trim()
        );
    }

    #[test]
    fn test_stringified_env() {
        assert_eq!(
            run(
                r#"log(A)"#,
                hashmap! {
                    "A".to_string() => json!("{\"v\": 1}")
                }
            ),
            "log(({\"v\": 1}));"
        );
    }

    #[test]
    fn test_dot_key() {
        assert_eq!(
            run(
                r#"log(x.y)"#,
                hashmap! {
                    "x.y".to_string() => json!(true)
                }
            ),
            "log(true);"
        );
    }

    #[test]
    fn test_deep_dot_key() {
        assert_eq!(
            run(
                r#"log(process.env.A)"#,
                hashmap! {
                    "process.env.A".to_string() => json!(true)
                }
            ),
            "log(true);"
        );
    }

    #[test]
    fn test_computed() {
        assert_eq!(
            run(
                r#"log(A["B"])"#,
                hashmap! {
                    "A.B".to_string() => json!(1)
                }
            ),
            "log(1);"
        );
    }

    #[test]
    fn test_computed_number() {
        assert_eq!(
            run(
                r#"log(A[1])"#,
                hashmap! {
                    "A.1".to_string() => json!(1)
                }
            ),
            "log(1);"
        );
    }

    #[test]
    fn test_computed_after_ident() {
        assert_eq!(
            run(
                r#"log(A.v["v"])"#,
                hashmap! {
                    "A.v.v".to_string() => json!(1)
                }
            ),
            "log(1);"
        );
    }

    #[test]
    fn test_computed_as_member_key() {
        assert_eq!(
            run(
                r#"log(A[v])"#,
                hashmap! {
                    "v".to_string() => json!(1)
                }
            ),
            "log(A[1]);"
        );
    }

    #[test]
    fn test_complicated_computed_as_member_key() {
        assert_eq!(
            run(
                r#"log(A[v.v])"#,
                hashmap! {
                    "v.v".to_string() => json!(1)
                }
            ),
            "log(A[1]);"
        );
    }

    #[test]
    fn test_computed_nested() {
        assert_eq!(
            run(
                r#"log(A[v].B[v][v].C[v][v][v])"#,
                hashmap! {
                    "v".to_string() => json!(1)
                }
            ),
            "log(A[1].B[1][1].C[1][1][1]);"
        );
    }

    #[test]
    fn test_should_not_replace_existed() {
        assert_eq!(
            run(
                r#"let v = 2;log(A[v])"#,
                hashmap! {
                    "v".to_string() => json!(1)
                }
            ),
            "let v = 2;log(A[v]);"
        );
    }

    #[test]
    fn test_should_not_replace_existed_as_member_prop() {
        assert_eq!(
            run(
                r#"let A = {};log(A.v, A[X.Y])"#,
                hashmap! {
                    "A".to_string() => json!("{\"v\": 1}"),
                    "X.Y".to_string() => json!(r#""xy""#)
                }
            ),
            r#"let A = {};log(A.v, A["xy"]);"#
        );
    }

    #[test]
    fn test_should_not_replace_not_defined() {
        assert_eq!(
            run(
                r#"log(A[B].v)"#,
                hashmap! {
                    "A.B".to_string() => json!("{\"v\": 1}")
                }
            ),
            "log(A[B].v);"
        );
    }

    #[test]
    fn test_should_not_replace_as_member_ident() {
        assert_eq!(
            run(
                r#"log(A.v)"#,
                hashmap! {
                    "v".to_string() => json!(1)
                }
            ),
            "log(A.v);"
        );
    }

    fn run(js_code: &str, envs: HashMap<String, Value>) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let envs = build_env_map(envs, &test_utils.context).unwrap();
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = EnvReplacer::new(envs, ast.unresolved_mark);
            ast.ast.visit_mut_with(&mut visitor);
        });
        let code = test_utils.js_ast_to_code();
        Regex::new(r"\s*\n\s*")
            .unwrap()
            .replace_all(&code, "")
            .to_string()
    }
}
