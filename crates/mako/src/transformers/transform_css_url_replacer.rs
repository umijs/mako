use std::sync::Arc;

use mako_core::swc_css_ast::{Url, UrlValue};
use mako_core::swc_css_visit::VisitMut;

use crate::ast_2::file::File;
use crate::ast_2::utils::{is_remote, remove_first_tilde};
use crate::compiler::Context;
use crate::load::Load;
use crate::module::Dependency;
use crate::resolve;

pub struct CSSUrlReplacer {
    pub context: Arc<Context>,
    pub path: String,
}

impl VisitMut for CSSUrlReplacer {
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

        if is_remote(&url) {
            return;
        }
        let url = remove_first_tilde(url);
        let dep = Dependency {
            source: url,
            resolve_as: None,
            resolve_type: crate::module::ResolveType::Css,
            order: 0,
            span: None,
        };
        let resolved = resolve::resolve(&self.path, &dep, &self.context.resolvers, &self.context);
        if let Ok(resource) = resolved {
            let resolved_path = resource.get_resolved_path();
            let asset_content = Load::handle_asset(
                &File::new(resolved_path.clone(), self.context.clone()),
                false,
                self.context.clone(),
            );
            let asset_content = asset_content.unwrap_or_else(|_| resolved_path);
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
