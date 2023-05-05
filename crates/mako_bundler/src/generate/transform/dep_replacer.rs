use std::collections::HashMap;
use swc_common::util::take::Take;
use swc_css_ast::{AtRulePrelude, Rule, Stylesheet, Url, UrlValue};
use swc_css_visit::{VisitMut as CssVisitMut, VisitMutWith as CssVisitMutWith};
use swc_ecma_ast::{Expr, ExprOrSpread, Lit, Str};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::build::analyze_deps::{is_commonjs_require, is_dynamic_import};

pub struct DepReplacer {
    pub dep_map: HashMap<String, String>,
}

impl VisitMut for DepReplacer {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_commonjs_require(call_expr) || is_dynamic_import(call_expr) {
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    self.replace_source(source);
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

impl CssVisitMut for DepReplacer {
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        n.rules = n
            .rules
            .take()
            .into_iter()
            .filter(|rule| match rule {
                Rule::AtRule(at_rule) => {
                    if let Some(box AtRulePrelude::ImportPrelude(_prelude)) = &at_rule.prelude {
                        // let href_string = match &prelude.href {
                        //   box ImportHref::Url(url) => {
                        //     let href_string = url
                        //       .value
                        //       .as_ref()
                        //       .map(|box value| match value {
                        //         UrlValue::Str(str) => str.value.clone(),
                        //         UrlValue::Raw(raw) => raw.value.clone(),
                        //       })
                        //       .unwrap_or_default();
                        //     href_string
                        //   }
                        //   box ImportHref::Str(str) => str.value.clone(),
                        // };
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            })
            .collect();
        n.visit_mut_children_with(self);
    }

    fn visit_mut_url(&mut self, n: &mut Url) {
        // 检查 url 属性
        match n.value {
            Some(box UrlValue::Str(ref mut s)) => {
                if let Some(replacement) = self.dep_map.get(&s.value.to_string()) {
                    s.value = replacement.clone().into();
                    s.raw = None;
                }
            }
            Some(box UrlValue::Raw(ref mut s)) => {
                if let Some(replacement) = self.dep_map.get(&s.value.to_string()) {
                    s.value = replacement.clone().into();
                    s.raw = None;
                }
            }
            None => {}
        };
        n.visit_mut_children_with(self);
    }
}

impl DepReplacer {
    fn replace_source(&mut self, source: &mut Str) {
        if let Some(replacement) = self.dep_map.get(&source.value.to_string()) {
            let span = source.span;

            // NOTE: JsWord 有缓存，直接设置 value 的方式在这种情况下不会生效
            // if (process.env.NODE_ENV === 'development') { require("./foo") }
            *source = Str::from(replacement.clone());
            // 保持原来的 span，不确定不加的话会不会导致 sourcemap 错误
            source.span = span;
        }
    }
}
