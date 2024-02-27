use mako_core::swc_css_ast::{ImportHref, UrlValue};
use mako_core::swc_css_visit::Visit;

use crate::ast_2::utils;
use crate::module::{Dependency, ResolveType};

pub struct CSSDepAnalyzer {
    pub dependencies: Vec<Dependency>,
    order: usize,
}

impl CSSDepAnalyzer {
    pub fn new() -> Self {
        Self {
            dependencies: vec![],
            // start with 1
            // 0 for swc helpers
            order: 1,
        }
    }

    fn add_dependency(&mut self, url: String) {
        if utils::is_remote(&url) {
            return;
        }
        let url = utils::remove_first_tilde(url);
        self.dependencies.push(Dependency {
            source: url,
            resolve_as: None,
            order: self.order,
            resolve_type: ResolveType::Css,
            span: None,
        });
        self.order += 1;
    }
}

impl Visit for CSSDepAnalyzer {
    fn visit_import_href(&mut self, n: &ImportHref) {
        match n {
            // e.g.
            // @import url(a.css)
            // @import url("a.css")
            ImportHref::Url(url) => {
                let src: Option<String> = url.value.as_ref().map(|box value| match value {
                    UrlValue::Str(str) => str.value.to_string(),
                    UrlValue::Raw(raw) => raw.value.to_string(),
                });
                if let Some(src) = src {
                    self.add_dependency(src);
                }
            }
            // e.g.
            // @import "a.css"
            ImportHref::Str(src) => {
                let src = src.value.to_string();
                self.add_dependency(src);
            }
        }
    }
}
