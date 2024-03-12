use mako_core::swc_common::util::take::Take;
use mako_core::swc_css_ast::{AtRule, AtRulePrelude, ImportHref, Rule, Stylesheet, UrlValue};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};

use crate::ast_2::utils::is_remote;

pub struct CssHandler;

impl VisitMut for CssHandler {
    // remove @import,
    // http(s) will not be removed
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        // move all @import to the top of other rules
        n.rules.sort_by_key(|rule| {
            let mut ret: i8 = 1;

            if let Rule::AtRule {
                0:
                    box AtRule {
                        prelude: Some(box AtRulePrelude::ImportPrelude(_)),
                        ..
                    },
                ..
            } = rule
            {
                ret = 0;
            }

            ret
        });

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
                        is_remote(&href_string)
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
