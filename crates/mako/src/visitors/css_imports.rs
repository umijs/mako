use swc_core::common::util::take::Take;
use swc_core::css::ast::{AtRule, AtRulePrelude, ImportHref, Rule, Stylesheet, UrlValue};
use swc_core::css::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::is_remote_or_data_or_hash;

pub struct CSSImports;

// TODO:
// 1. hoist and remove relative could be done in separate visitors
// 2. hoist could be done in transform phase
impl VisitMut for CSSImports {
    fn visit_mut_stylesheet(&mut self, n: &mut Stylesheet) {
        // hoist all import
        // if import is not hoisted, it's not invalid in browser render
        // relative imports will be removed later
        // so only non-relative imports make sense here
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

        // keep non-relative imports
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
                        is_remote_or_data_or_hash(&href_string)
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

#[cfg(test)]
mod tests {
    use swc_core::css::visit::VisitMutWith;

    use crate::ast::tests::TestUtils;

    #[test]
    fn test_keep_none_relative() {
        assert_eq!(
            run(r#"
@import url(//a);
@import url(http://a);
@import url(https://a);
@import url(data://a);
@import url(#a);
@import url(a.css);
@import url(b.css);
                "#),
            r#"
@import url(//a);
@import url(http://a);
@import url(https://a);
@import url(data://a);
@import url(#a);
                "#
            .trim()
        );
    }

    #[test]
    fn test_hoist_imports() {
        assert_eq!(
            run(r#"
.a {}
@import url(//a);
.b {}
@import url(//b);
                    "#),
            r#"
@import url(//a);
@import url(//b);
.a {}
.b {}
                    "#
            .trim()
        );
    }

    fn run(css_code: &str) -> String {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), false);
        let ast = test_utils.ast.css_mut();
        let mut visitor = super::CSSImports {};
        ast.ast.visit_mut_with(&mut visitor);
        test_utils.css_ast_to_code()
    }
}
