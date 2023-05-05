use std::collections::HashMap;
use swc_css_ast::{ImportHref, Url, UrlValue};
use swc_css_visit::VisitMut as CssVisitMut;
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
    fn visit_mut_import_href(&mut self, n: &mut ImportHref) {
        // 检查 @import
        if let ImportHref::Url(url) = n {
            let href_string = url
                .value
                .as_ref()
                .map(|box value| match value {
                    UrlValue::Str(str) => str.value.to_string(),
                    UrlValue::Raw(raw) => raw.value.to_string(),
                })
                .unwrap();
        } else if let ImportHref::Str(str) = n {
        }
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
