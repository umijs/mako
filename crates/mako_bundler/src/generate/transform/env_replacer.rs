use swc_atoms::{js_word, JsWord};
use swc_common::{
    collections::{AHashMap, AHashSet},
    sync::Lrc,
    DUMMY_SP,
};
use swc_ecma_ast::{
    ComputedPropName, Expr, Id, Ident, KeyValueProp, Lit, MemberExpr, MemberProp, MetaPropExpr,
    MetaPropKind, Module, ObjectLit, Prop, PropName, PropOrSpread, Str,
};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

enum EnvsType {
    Node(Lrc<AHashMap<JsWord, Expr>>),
    Browser(Lrc<AHashMap<String, Expr>>),
}

pub struct EnvReplacer {
    bindings: Lrc<AHashSet<Id>>,
    envs: Lrc<AHashMap<JsWord, Expr>>,
    meta_envs: Lrc<AHashMap<String, Expr>>,
}
impl EnvReplacer {
    pub fn new(envs: Lrc<AHashMap<JsWord, Expr>>) -> Self {
        let mut meta_env_map = AHashMap::default();

        // generate meta_envs from envs
        for (k, v) in envs.iter() {
            // convert NODE_ENV to MODE
            let key = if k.eq(&js_word!("NODE_ENV")) {
                "MODE".into()
            } else {
                k.to_string()
            };

            meta_env_map.insert(key, v.clone());
        }

        Self {
            bindings: Default::default(),
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
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.bindings = Lrc::new(collect_decls(&*module));

        module.visit_mut_children_with(self);
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            if self.bindings.contains(&(sym.clone(), span.ctxt)) {
                return;
            }
        }

        if let Expr::Member(MemberExpr { obj, prop, .. }) = expr {
            if let Expr::Member(MemberExpr {
                obj: first_obj,
                prop:
                    MemberProp::Ident(Ident {
                        sym: js_word!("env"),
                        ..
                    }),
                ..
            }) = &**obj
            {
                // handle `env.XX`
                let mut envs = EnvsType::Node(self.envs.clone());

                if match &**first_obj {
                    Expr::Ident(Ident {
                        sym: js_word!("process"),
                        ..
                    }) => true,
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
                    // create an empty object to replace below
                    let expr_with_empty_obj = Expr::Member(MemberExpr {
                        obj: Box::new(Expr::Object(ObjectLit {
                            span: DUMMY_SP,
                            props: vec![],
                        })),
                        prop: prop.clone(),
                        span: DUMMY_SP,
                    });

                    match prop {
                        MemberProp::Computed(ComputedPropName { expr: c, .. }) => {
                            if let Expr::Lit(Lit::Str(Str { value: sym, .. })) = &**c {
                                if let Some(env) = EnvReplacer::get_env(&envs, sym) {
                                    // replace with real value if env found
                                    *expr = env;
                                } else {
                                    // replace with an empty object if env not found
                                    *expr = expr_with_empty_obj;
                                }
                            }
                        }

                        MemberProp::Ident(Ident { sym, .. }) => {
                            if let Some(env) = EnvReplacer::get_env(&envs, sym) {
                                // replace with real value if env found
                                *expr = env;
                            } else {
                                // replace with an empty object if env not found
                                *expr = expr_with_empty_obj;
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Expr::Member(MemberExpr {
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
            }) = *expr
            {
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
