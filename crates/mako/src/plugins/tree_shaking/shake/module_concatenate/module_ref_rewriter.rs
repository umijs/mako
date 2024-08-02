use std::collections::HashSet;

use swc_core::common::util::take::Take;
use swc_core::common::{Span, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::transforms::base::helpers::HELPERS;
use swc_core::ecma::utils::{
    is_valid_prop_ident, quote_ident, quote_str, ExprFactory, IntoIndirectCall,
};
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use super::concatenate_context::ImportModuleRefMap;
use crate::DUMMY_CTXT;

pub struct ModuleRefRewriter<'a> {
    /// ```javascript
    /// import foo, { a as b, c } from "mod";
    /// import * as x from "x";
    /// foo, b, c;
    /// x;
    /// ```
    /// ->
    /// ```javascript
    /// _mod.default, _mod.a, _mod.c;
    /// _x;
    ///
    /// Map(
    ///     foo => (_mod, Some("default")),
    ///     b => (_mod, Some("a")),
    ///     c => (_mod, Some("c")),
    ///     x => (_x, None),
    /// )
    /// ```
    pub import_map: &'a ImportModuleRefMap,

    pub lazy_record: HashSet<Id>,

    pub allow_top_level_this: bool,

    is_global_this: bool,
    helper_ctxt: Option<SyntaxContext>,
}

impl<'a> ModuleRefRewriter<'a> {
    pub fn new(
        import_map: &'a ImportModuleRefMap,
        lazy_record: HashSet<Id>,
        allow_top_level_this: bool,
    ) -> Self {
        Self {
            import_map,
            lazy_record,
            allow_top_level_this,
            is_global_this: true,
            helper_ctxt: {
                HELPERS
                    .is_set()
                    .then(|| HELPERS.with(|helper| helper.mark()))
                    .map(|mark| SyntaxContext::empty().apply_mark(mark))
            },
        }
    }
}

impl VisitMut for ModuleRefRewriter<'_> {
    noop_visit_mut_type!();

    /// replace bar in binding pattern
    /// const foo = { bar }
    fn visit_mut_prop(&mut self, n: &mut Prop) {
        match n {
            Prop::Shorthand(shorthand) => {
                if let Some(expr) = self.map_module_ref_ident(shorthand) {
                    *n = KeyValueProp {
                        key: shorthand.take().into(),
                        value: Box::new(expr),
                    }
                    .into()
                }
            }
            _ => n.visit_mut_children_with(self),
        }
    }

    fn visit_mut_expr(&mut self, n: &mut Expr) {
        match n {
            Expr::Ident(ref_ident) => {
                if let Some(expr) = self.map_module_ref_ident(ref_ident) {
                    *n = expr;
                }
            }

            Expr::This(ThisExpr { span }) => {
                if !self.allow_top_level_this && self.is_global_this {
                    *n = *Expr::undefined(*span);
                }
            }

            _ => n.visit_mut_children_with(self),
        };
    }

    fn visit_mut_callee(&mut self, n: &mut Callee) {
        match n {
            Callee::Expr(e) if e.is_ident() => {
                let is_indirect_callee = e
                    .as_ident()
                    .filter(|ident| self.helper_ctxt.iter().all(|ctxt| ctxt != &ident.ctxt))
                    .and_then(|ident| self.import_map.get(&ident.to_id()))
                    .map(|(_, prop)| prop.is_some())
                    .unwrap_or_default();

                e.visit_mut_with(self);

                if is_indirect_callee {
                    *n = n.take().into_indirect()
                }
            }

            _ => n.visit_mut_children_with(self),
        }
    }

    fn visit_mut_tagged_tpl(&mut self, n: &mut TaggedTpl) {
        let is_indirect = n
            .tag
            .as_ident()
            .filter(|ident| self.helper_ctxt.iter().all(|ctxt| ctxt != &ident.ctxt))
            .and_then(|ident| self.import_map.get(&ident.to_id()))
            .map(|(_, prop)| prop.is_some())
            .unwrap_or_default();

        n.visit_mut_children_with(self);

        if is_indirect {
            *n = n.take().into_indirect()
        }
    }

    fn visit_mut_function(&mut self, n: &mut Function) {
        self.visit_mut_with_non_global_this(n);
    }

    fn visit_mut_constructor(&mut self, n: &mut Constructor) {
        self.visit_mut_with_non_global_this(n);
    }

    fn visit_mut_class_prop(&mut self, n: &mut ClassProp) {
        n.key.visit_mut_with(self);

        self.visit_mut_with_non_global_this(&mut n.value);
    }

    fn visit_mut_private_prop(&mut self, n: &mut PrivateProp) {
        n.key.visit_mut_with(self);

        self.visit_mut_with_non_global_this(&mut n.value);
    }

    fn visit_mut_getter_prop(&mut self, n: &mut GetterProp) {
        n.key.visit_mut_with(self);

        self.visit_mut_with_non_global_this(&mut n.body);
    }

    fn visit_mut_setter_prop(&mut self, n: &mut SetterProp) {
        n.key.visit_mut_with(self);

        self.visit_mut_with_non_global_this(&mut n.body);
    }

    fn visit_mut_static_block(&mut self, n: &mut StaticBlock) {
        self.visit_mut_with_non_global_this(n);
    }
}

impl ModuleRefRewriter<'_> {
    fn visit_mut_with_non_global_this<T>(&mut self, n: &mut T)
    where
        T: VisitMutWith<Self>,
    {
        let top_level = self.is_global_this;

        self.is_global_this = false;
        n.visit_mut_children_with(self);
        self.is_global_this = top_level;
    }

    fn map_module_ref_ident(&mut self, ref_ident: &Ident) -> Option<Expr> {
        self.import_map
            .get(&ref_ident.to_id())
            .map(|(mod_ident, mod_prop)| -> Expr {
                let mut mod_ident = mod_ident.clone();
                let span = ref_ident.span;
                mod_ident.span = span;

                let mod_expr = if self.lazy_record.contains(&mod_ident.to_id()) {
                    mod_ident.as_call(span, Default::default())
                } else {
                    mod_ident.into()
                };

                if let Some(imported_name) = mod_prop {
                    let prop = prop_name(imported_name, DUMMY_SP).into();

                    MemberExpr {
                        obj: Box::new(mod_expr),
                        span,
                        prop,
                    }
                    .into()
                } else {
                    mod_expr
                }
            })
    }
}

fn prop_name(key: &str, span: Span) -> IdentOrStr {
    if is_valid_prop_ident(key) {
        IdentOrStr::Ident(quote_ident!(DUMMY_CTXT, span, key))
    } else {
        IdentOrStr::Str(quote_str!(span, key))
    }
}

enum IdentOrStr {
    Ident(Ident),
    Str(Str),
}

impl From<IdentOrStr> for PropName {
    fn from(val: IdentOrStr) -> Self {
        match val {
            IdentOrStr::Ident(i) => Self::Ident(i.into()),
            IdentOrStr::Str(s) => Self::Str(s),
        }
    }
}

impl From<IdentOrStr> for MemberProp {
    fn from(val: IdentOrStr) -> Self {
        match val {
            IdentOrStr::Ident(i) => Self::Ident(i.into()),
            IdentOrStr::Str(s) => Self::Computed(ComputedPropName {
                span: DUMMY_SP,
                expr: s.into(),
            }),
        }
    }
}
