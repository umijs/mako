use std::sync::Arc;

use mako_core::swc_css_ast::{Url, UrlValue};
use mako_core::swc_css_visit::VisitMut;

use crate::ast::file::File;
use crate::ast::utils::{is_remote_or_data_or_hash, remove_first_tilde};
use crate::build::load::Load;
use crate::compiler::Context;
use crate::module::{Dependency, ResolveType};
use crate::resolve;

pub struct CSSAssets {
    pub context: Arc<Context>,
    pub path: String,
}

impl VisitMut for CSSAssets {
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

        if is_remote_or_data_or_hash(&url) {
            return;
        }
        let url = remove_first_tilde(url);
        let dep = Dependency {
            source: url,
            resolve_as: None,
            resolve_type: ResolveType::Css,
            order: 0,
            span: None,
        };
        let resolved = resolve::resolve(&self.path, &dep, &self.context.resolvers, &self.context);
        if let Ok(resource) = resolved {
            let resolved_path = resource.get_resolved_path();
            let asset_content = Load::handle_asset(
                &File::new(resolved_path.clone(), self.context.clone()),
                false,
                true,
                self.context.clone(),
            );
            let asset_content = asset_content.unwrap_or(resolved_path);
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

#[cfg(test)]
mod tests {
    use mako_core::swc_css_visit::VisitMutWith;

    use crate::ast::tests::TestUtils;

    #[test]
    fn test_base64() {
        assert!(run(r#".foo { background: url(umi.png) }"#)
            .contains(".foo{background:url(data:image/png;base64,"));
    }

    #[test]
    fn test_strip_first_slash() {
        assert!(run(r#".foo { background: url(~umi.png) }"#)
            .contains(".foo{background:url(data:image/png;base64,"));
    }

    #[test]
    fn test_remote() {
        assert_eq!(
            run(r#".foo { background: url(https://a.png) }"#),
            ".foo{background:url(https://a.png)}"
        );
        assert_eq!(
            run(r#".foo { background: url(http://a.png) }"#),
            ".foo{background:url(http://a.png)}"
        );
        assert_eq!(
            run(r#".foo { background: url(//a.png) }"#),
            ".foo{background:url(//a.png)}"
        );
        assert_eq!(
            run(r#".foo { background: url(data://a.png) }"#),
            ".foo{background:url(data://a.png)}"
        );
    }

    #[test]
    fn test_not_found() {
        assert_eq!(
            run(r#".foo { background: url(should-not-exists.png) }"#),
            ".foo{background:url(should-not-exists.png)}"
        );
    }

    #[test]
    fn test_big_image() {
        assert!(run(r#".foo { background: url(big.jpg) }"#).contains(".foo{background:url(big."));
    }

    fn run(css_code: &str) -> String {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), true);
        let ast = test_utils.ast.css_mut();
        let current_dir = std::env::current_dir().unwrap();
        let css_path = current_dir.join("src/visitors/fixtures/css_assets/test.css");
        let mut visitor = super::CSSAssets {
            context: test_utils.context.clone(),
            path: css_path.to_string_lossy().to_string(),
        };
        ast.ast.visit_mut_with(&mut visitor);
        test_utils.css_ast_to_code()
    }
}
