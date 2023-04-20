use swc_atoms::{js_word, JsWord};
use swc_common::{
    collections::{AHashMap, AHashSet},
    sync::Lrc,
};
use swc_ecma_ast::{ComputedPropName, Expr, Id, Ident, Lit, MemberExpr, MemberProp, Module, Str};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct EnvReplacer {
    bindings: Lrc<AHashSet<Id>>,
    envs: Lrc<AHashMap<JsWord, Expr>>,
}
impl EnvReplacer {
    pub fn new(envs: Lrc<AHashMap<JsWord, Expr>>) -> Self {
        Self {
            bindings: Default::default(),
            envs,
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

        expr.visit_mut_children_with(self);

        match expr {
            Expr::Member(MemberExpr { obj, prop, .. }) => {
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
                    if let Expr::Ident(Ident {
                        sym: js_word!("process"),
                        ..
                    }) = &**first_obj
                    {
                        match prop {
                            MemberProp::Computed(ComputedPropName { expr: c, .. }) => {
                                if let Expr::Lit(Lit::Str(Str { value: sym, .. })) = &**c {
                                    if let Some(env) = self.envs.get(sym) {
                                        *expr = env.clone();
                                    }
                                }
                            }

                            MemberProp::Ident(Ident { sym, .. }) => {
                                if let Some(env) = self.envs.get(sym) {
                                    *expr = env.clone();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
