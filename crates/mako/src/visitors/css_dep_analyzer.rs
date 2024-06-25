use swc_core::css::ast::{ImportHref, UrlValue};
use swc_core::css::visit::Visit;

use crate::ast::utils;
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
        if utils::is_remote_or_data_or_hash(&url) {
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

#[cfg(test)]
mod tests {
    use swc_core::css::visit::VisitWith;

    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(run(r#"@import url(a.css);"#), vec!["a.css"]);
        assert_eq!(run(r#"@import url("a.css");"#), vec!["a.css"]);
        assert_eq!(run(r#"@import url('a.css');"#), vec!["a.css"]);
        assert_eq!(run(r#"@import "a.css";"#), vec!["a.css"]);
        assert_eq!(run(r#"@import 'a.css';"#), vec!["a.css"]);
    }

    #[test]
    fn test_with_tidle() {
        assert_eq!(run(r#"@import url(~a.css);"#), vec!["a.css"]);
        assert_eq!(run(r#"@import url("~a.css");"#), vec!["a.css"]);
        assert_eq!(run(r#"@import url('~a.css');"#), vec!["a.css"]);
        assert_eq!(run(r#"@import "~a.css";"#), vec!["a.css"]);
        assert_eq!(run(r#"@import '~a.css';"#), vec!["a.css"]);
        // ignore ~/
        assert_eq!(run(r#"@import url(~/a.css);"#), vec!["~/a.css"]);
    }

    #[test]
    fn test_remote() {
        assert!(run(r#"@import url(https://a.com/a.css);"#).is_empty());
        assert!(run(r#"@import url(http://a.com/a.css);"#).is_empty());
        assert!(run(r#"@import url(data://a.com/a.css);"#).is_empty());
        assert!(run(r#"@import url(//a.com/a.css);"#).is_empty());
        assert!(run(r#"@import url(#a);"#).is_empty());
    }

    #[test]
    fn test_multiple() {
        assert_eq!(
            run(r#"
            @import url(a.css);
            @import url(b.css);
            "#),
            vec!["a.css", "b.css"]
        );
    }

    fn run(css_code: &str) -> Vec<String> {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), false);
        let ast = test_utils.ast.css_mut();
        let mut analyzer = super::CSSDepAnalyzer::new();
        ast.ast.visit_with(&mut analyzer);
        let sources = analyzer
            .dependencies
            .iter()
            .map(|dep| dep.source.clone())
            .collect();
        sources
    }
}
