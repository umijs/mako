use std::sync::Arc;

use swc_css_ast::{Url, UrlValue};
use swc_css_visit::VisitMut;

use crate::analyze_deps::{handle_css_url, is_url_ignored};
use crate::compiler::Context;
use crate::load::handle_asset;
use crate::module::Dependency;
use crate::resolve::{self, Resolvers};

pub struct CSSUrlReplacer<'a> {
    pub resolvers: &'a Resolvers,
    pub context: &'a Arc<Context>,
    pub path: &'a str,
}

impl VisitMut for CSSUrlReplacer<'_> {
    // e.g.
    // .foo { background: url(foo.png) }
    fn visit_mut_url(&mut self, n: &mut Url) {
        if n.value.is_none() {
            return;
        }
        let value = n.value.as_ref().unwrap();
        let url = match value {
            box UrlValue::Str(s) => s.value.to_string(),
            box UrlValue::Raw(s) => s.value.to_string(),
        };

        if is_url_ignored(&url) {
            return;
        }
        let url = handle_css_url(url);
        let dep = Dependency {
            source: url,
            resolve_type: crate::module::ResolveType::Css,
            order: 0,
        };
        let resolved = resolve::resolve(self.path, &dep, self.resolvers, self.context);
        if let Ok((resolved_path, _)) = resolved {
            let asset_content = handle_asset(self.context, &resolved_path, false);
            let asset_content = asset_content.unwrap_or_else(|_| resolved_path.clone());
            match n.value {
                Some(box UrlValue::Str(ref mut s)) => {
                    s.value = asset_content.into();
                    s.raw = None;
                }
                Some(box UrlValue::Raw(ref mut s)) => {
                    s.value = asset_content.into();
                    s.raw = None;
                }
                None => {}
            }
        }
    }
}
