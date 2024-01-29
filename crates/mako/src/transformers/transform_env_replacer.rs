use std::collections::HashMap;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::serde_json::Value;
use mako_core::swc_atoms::{js_word, JsWord};
use mako_core::swc_common::collections::AHashMap;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    ArrayLit, Bool, ComputedPropName, Expr, ExprOrSpread, Ident, KeyValueProp, Lit, MemberExpr,
    MemberProp, MetaPropExpr, MetaPropKind, ModuleItem, Null, Number, ObjectLit, Prop, PropName,
    PropOrSpread, Stmt, Str,
};
use mako_core::swc_ecma_utils::{quote_ident, ExprExt};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_core::common::Mark;

use crate::ast::build_js_ast;
use crate::compiler::Context;
use crate::config::ConfigError;

enum EnvsType {
    Node(Lrc<AHashMap<JsWord, Expr>>),
    Browser(Lrc<AHashMap<String, Expr>>),
}

#[derive(Debug)]
pub struct EnvReplacer {
    unresolved_mark: Mark,
    envs: Lrc<AHashMap<JsWord, Expr>>,
    meta_envs: Lrc<AHashMap<String, Expr>>,
}
impl EnvReplacer {
    pub fn new(envs: Lrc<AHashMap<JsWord, Expr>>, unresolved_mark: Mark) -> Self {
        let mut meta_env_map = AHashMap::default();

        // generate meta_envs from envs
        for (k, v) in envs.iter() {
            // convert NODE_ENV to MODE
            let key: String = if k.eq(&js_word!("NODE_ENV")) {
                "MODE".into()
            } else {
                k.to_string()
            };

            meta_env_map.insert(key, v.clone());
        }

        Self {
            unresolved_mark,
            envs,
            meta_envs: Lrc::new(meta_env_map),
        }
    }

    fn get_env(envs: &EnvsType, sym: &JsWord) -> Option<Expr> {
        match envs {
            EnvsType::Node(envs) => envs.get(sym).cloned(),
            EnvsType::Browser(envs) => envs.get(&sym.to_string()).cloned(),
        }
    }
}
impl VisitMut for EnvReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let envs = EnvsType::Node(self.envs.clone());

            // 先判断 env 中的变量名称，是否是上下文中已经存在的变量名称
            if span.ctxt.outer() != self.unresolved_mark {
                expr.visit_mut_children_with(self);
                return;
            }

            if let Some(env) = EnvReplacer::get_env(&envs, sym) {
                // replace with real value if env found
                *expr = env;
                return;
            }
        }

        if let Expr::Member(MemberExpr {
            obj: box obj, prop, ..
        }) = expr
        {
            if match obj {
                Expr::Member(MemberExpr {
                    prop: MemberProp::Ident(Ident { sym, .. }),
                    ..
                }) => sym == "env",
                _ => false,
            } {
                // handle `env.XX`
                let mut envs = EnvsType::Node(self.envs.clone());

                if let Expr::Member(MemberExpr {
                    obj: box first_obj, ..
                }) = obj
                {
                    if match first_obj {
                        Expr::Ident(Ident { sym, .. }) => sym == "process",
                        Expr::MetaProp(MetaPropExpr {
                            kind: MetaPropKind::ImportMeta,
                            ..
                        }) => {
                            envs = EnvsType::Browser(self.meta_envs.clone());
                            true
                        }
                        _ => false,
                    } {
                        // handle `process.env.XX` and `import.meta.env.XX`
                        match prop {
                            MemberProp::Computed(ComputedPropName { expr: c, .. }) => {
                                if let Expr::Lit(Lit::Str(Str { value: sym, .. })) = &**c {
                                    if let Some(env) = EnvReplacer::get_env(&envs, sym) {
                                        // replace with real value if env found
                                        *expr = env;
                                    } else {
                                        // replace with `undefined` if env not found
                                        *expr = *Box::new(Expr::Ident(Ident::new(
                                            js_word!("undefined"),
                                            DUMMY_SP,
                                        )));
                                    }
                                }
                            }

                            MemberProp::Ident(Ident { sym, .. }) => {
                                if let Some(env) = EnvReplacer::get_env(&envs, sym) {
                                    // replace with real value if env found
                                    *expr = env;
                                } else {
                                    // replace with `undefined` if env not found
                                    *expr = *Box::new(Expr::Ident(Ident::new(
                                        js_word!("undefined"),
                                        DUMMY_SP,
                                    )));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else if match expr {
                Expr::Member(MemberExpr {
                    obj:
                        box Expr::MetaProp(MetaPropExpr {
                            kind: MetaPropKind::ImportMeta,
                            ..
                        }),
                    prop: MemberProp::Ident(Ident { sym, .. }),
                    ..
                }) => sym == "env",
                _ => false,
            } {
                // replace independent `import.meta.env` to json object
                let mut props = Vec::new();

                // convert envs to object properties
                for (k, v) in self.meta_envs.iter() {
                    props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                        key: PropName::Ident(Ident::new(k.clone().into(), DUMMY_SP)),
                        value: Box::new(v.clone()),
                    }))));
                }

                *expr = Expr::Object(ObjectLit {
                    span: DUMMY_SP,
                    props,
                });
            }
        }

        expr.visit_mut_children_with(self);
    }
}

pub fn build_env_map(
    env_map: HashMap<String, Value>,
    context: &Arc<Context>,
) -> Result<AHashMap<JsWord, Expr>> {
    let mut map = AHashMap::default();
    for (k, v) in env_map.into_iter() {
        let expr = get_env_expr(v, context)?;
        map.insert(k.into(), expr);
    }
    Ok(map)
}

fn get_env_expr(v: Value, context: &Arc<Context>) -> Result<Expr> {
    match v {
        Value::String(v) => {
            // the string content is treat as expression, so it has to be parsed
            let ast = build_js_ast("_define_.js", &v, context).unwrap();
            let module = ast.ast.body.first().unwrap();

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::serde_json::json;
    use mako_core::swc_common::{Globals, GLOBALS};
    use mako_core::swc_ecma_visit::VisitMutWith;
    use maplit::hashmap;

    use super::EnvReplacer;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;
    use crate::transformers::transform_env_replacer::build_env_map;

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
    fn test_transform_undefined_env() {
        let code = r#"
if (process.env.UNDEFINED_ENV === 'true') {
    console.log('UNDEFINED env is true');
}
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
if (undefined === 'true') {
    console.log('UNDEFINED env is true');
}

//# sourceMappingURL=index.js.map
                    "#
            .trim()
        );
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context: Arc<Context> = Arc::new(Default::default());

        let mut ast = build_js_ast(path, origin, &context).unwrap();

        let globals = Globals::default();
        GLOBALS.set(&globals, || {
            let mut env_replacer = EnvReplacer::new(Default::default(), ast.unresolved_mark);
            ast.ast.visit_mut_with(&mut env_replacer);
        });

        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
