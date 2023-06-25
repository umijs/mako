use std::collections::HashMap;

use swc_common::util::take::Take;
use swc_css_ast::{AtRulePrelude, Rule, Stylesheet, Url, UrlValue};
use swc_css_visit::{VisitMut, VisitMutWith};

pub struct CssHandler {
    pub dep_map: HashMap<String, String>,
}

impl VisitMut for CssHandler {
    // remove @import
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        n.rules = n
            .rules
            .take()
            .into_iter()
            .filter(|rule| match rule {
                Rule::AtRule(at_rule) => !matches!(
                    &at_rule.prelude,
                    Some(box AtRulePrelude::ImportPrelude(_prelude))
                ),
                _ => true,
            })
            .collect();
        n.visit_mut_children_with(self);
    }

    // replace url()
    fn visit_mut_url(&mut self, n: &mut Url) {
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
        // n.visit_mut_children_with(self);
    }
}
