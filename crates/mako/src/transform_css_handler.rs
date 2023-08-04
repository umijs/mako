use std::cmp::Reverse;

use swc_common::util::take::Take;
use swc_css_ast::{AtRulePrelude, ImportHref, Rule, Stylesheet, UrlValue};
use swc_css_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::is_url_ignored;

pub struct CssHandler;

impl VisitMut for CssHandler {
    // remove @import,
    // http(s) will not be removed
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        // move all @import to the top of other rules
        n.rules.sort_by_key(|rule| Reverse(rule.is_at_rule() as i8));

        // filter non-url @import
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
}
