use std::collections::HashMap;
use std::sync::Arc;

use swc_common::util::take::Take;
use swc_css_ast::{AtRulePrelude, ImportHref, Rule, Stylesheet, Url, UrlValue};
use swc_css_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::{handle_css_url, is_url_ignored};
use crate::compiler::Context;
use crate::load::handle_asset;

pub struct CssHandler<'a> {
    pub assets_map: HashMap<String, String>,
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
                if is_url_ignored(url) {
                    return;
                }
                let url = handle_css_url(url.to_string());
                println!(
                    "self.dep_map: {:?}, url: {}, exists: {}",
                    self.assets_map,
                    &url,
                    self.assets_map.get(&url).is_some()
                );
                if let Some(replacement) = self.assets_map.get(&url) {
                    // CSS url() 里的资源是 css visit 时 handle 的
                    let asset_content = handle_asset(self.context, replacement);
                    s.value = asset_content.unwrap_or_else(|_| replacement.clone()).into();
                    println!("s.value: {}", &s.value);
                    s.raw = None;
                }
            }
            Some(box UrlValue::Raw(ref mut s)) => {
                let url = &s.value.to_string();
                if is_url_ignored(url) {
                    return;
                }
                let url = handle_css_url(url.to_string());
                if let Some(replacement) = self.assets_map.get(&url) {
                    // CSS url() 里的资源是 css visit 时 handle 的
                    let asset_content = handle_asset(self.context, replacement);
                    s.value = asset_content.unwrap_or_else(|_| replacement.clone()).into();
                    s.raw = None;
                }
            }
            None => {}
        };
        // n.visit_mut_children_with(self);
    }
}
