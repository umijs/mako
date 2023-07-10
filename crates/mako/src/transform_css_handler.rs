use std::collections::HashMap;
use std::sync::Arc;

use swc_common::util::take::Take;
use swc_css_ast::{AtRulePrelude, ImportHref, Rule, Stylesheet, Url, UrlValue};
use swc_css_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::is_url_ignored;
use crate::compiler::Context;
use crate::module::generate_module_id;

pub struct CssHandler<'a> {
    pub dep_map: HashMap<String, String>,
    pub context: &'a Arc<Context>,
}

impl VisitMut for CssHandler<'_> {
    // remove @import,
    // http(s) will not be removed
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        n.rules = n
            .rules
            .take()
            .into_iter()
            .filter(|rule| match rule {
                Rule::AtRule(at_rule) => {
                    if let Some(box AtRulePrelude::ImportPrelude(prelude)) = &at_rule.prelude {
                        let href_string = match &prelude.href {
                            box ImportHref::Url(url) => {
                                let href_string = url
                                    .value
                                    .as_ref()
                                    .map(|box value| match value {
                                        UrlValue::Str(str) => str.value.to_string(),
                                        UrlValue::Raw(raw) => raw.value.to_string(),
                                    })
                                    .unwrap_or_default();
                                href_string
                            }
                            box ImportHref::Str(str) => str.value.to_string(),
                        };
                        is_url_ignored(&href_string)
                    } else {
                        true
                    }
                }
                _ => true,
            })
            .collect();
        n.visit_mut_children_with(self);
    }

    // replace url()
    fn visit_mut_url(&mut self, n: &mut Url) {
        match n.value {
            Some(box UrlValue::Str(ref mut s)) => {
                let url = &s.value.to_string();
                if !is_url_ignored(url) {
                    if let Some(replacement) = self.dep_map.get(url) {
                        s.value = generate_module_id(replacement.clone(), self.context).into();
                        s.raw = None;
                    }
                }
            }
            Some(box UrlValue::Raw(ref mut s)) => {
                let url = &s.value.to_string();
                if !is_url_ignored(url) {
                    if let Some(replacement) = self.dep_map.get(url) {
                        s.value = generate_module_id(replacement.clone(), self.context).into();
                        s.raw = None;
                    }
                }
            }
            None => {}
        };
        // n.visit_mut_children_with(self);
    }
}
