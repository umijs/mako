use mako_core::regex::Regex;
use mako_core::swc_css_ast::{self, Length, Token};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};
use swc_core::css::ast::{CompoundSelector, SubclassSelector};

use crate::config::Px2RemConfig;

pub(crate) fn default_root() -> f64 {
    100.0
}

pub struct Px2Rem {
    pub config: Px2RemConfig,
    pub current_decl: Option<String>,
    pub current_selector: Option<String>,
}

impl Px2Rem {
    pub fn new(config: Px2RemConfig) -> Self {
        Self {
            config,
            current_decl: None,
            current_selector: None,
        }
    }

    fn should_transform(&self) -> bool {
        let is_whitelist_is_in_blacklist = if let Some(decl) = &self.current_decl {
            let is_whitelist_empty = self.config.prop_white_list.is_empty();
            let is_in_whitelist = self.config.prop_white_list.contains(decl);
            let is_in_blacklist = self.config.prop_black_list.contains(decl);
            (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist
        } else {
            true
        };
        let is_select_black_is_select_white = if let Some(selector) = &self.current_selector {
            let is_whitelist_empty = self.config.selector_white_list.is_empty();
            let is_in_whitelist = self.config.selector_white_list.iter().any(|pattern| {
                let re = Regex::new(pattern).unwrap();
                re.is_match(selector)
            });
            let is_in_blacklist = self.config.selector_black_list.iter().any(|pattern| {
                // TODO: should have performance issues, need benchmark
                let re = Regex::new(pattern).unwrap();
                re.is_match(selector)
            });
            (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist
        } else {
            true
        };
        is_whitelist_is_in_blacklist && is_select_black_is_select_white
    }
}

impl VisitMut for Px2Rem {
    fn visit_mut_declaration(&mut self, n: &mut swc_css_ast::Declaration) {
        self.current_decl = match n.name {
            swc_css_ast::DeclarationName::Ident(ref ident) => Some(ident.value.to_string()),
            swc_css_ast::DeclarationName::DashedIdent(ref dashed_ident) => {
                Some(dashed_ident.value.to_string())
            }
        };
        n.visit_mut_children_with(self);
        self.current_decl = None;
    }

    fn visit_mut_compound_selector(&mut self, n: &mut CompoundSelector) {
        if !n.subclass_selectors.is_empty() {
            // 假设我们只关心第一个 subclass_selector，因此获取第一个元素

            // 使用匹配来处理不同类型的 SubclassSelector
            self.current_selector = match n.subclass_selectors.first().unwrap() {
                SubclassSelector::Class(class_selector) => {
                    Some(class_selector.text.value.clone().to_string())
                }
                // ... 处理 SubclassSelector 的其他变体 ...
                _ => None,
            };
        } else if let Some(type_selector) = &n.type_selector {
            self.current_selector = Some(
                type_selector
                    .clone()
                    .tag_name()
                    .unwrap()
                    .name
                    .value
                    .value
                    .to_string(),
            )
        } else {
            self.current_selector = Some(String::from(""));
        }
        n.visit_mut_children_with(self);
    }

    fn visit_mut_length(&mut self, n: &mut Length) {
        if n.unit.value.to_string() == "px" && self.should_transform() {
            n.value.value /= self.config.root;
            n.value.raw = None;
            n.unit.value = "rem".into();
        }
        n.visit_mut_children_with(self);
    }

    fn visit_mut_token(&mut self, t: &mut Token) {
        if let Token::Dimension(dimension) = t {
            if dimension.unit.to_string() == "px" && self.should_transform() {
                let rem_val = dimension.value / self.config.root;
                dimension.raw_value = rem_val.to_string().into();
                dimension.value = rem_val;
                dimension.raw_unit = "rem".into();
                dimension.unit = "rem".into();
            }
        }
        t.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use mako_core::swc_css_visit::VisitMutWith;

    use super::Px2Rem;
    use crate::ast_2::tests::TestUtils;
    use crate::config::Px2RemConfig;

    #[test]
    fn test_normal() {
        assert_eq!(
            run_with_default(r#".a{width:100px;height:200px;}"#),
            r#".a{width:1rem;height:2rem}"#
        );
    }

    #[test]
    fn test_media_query() {
        assert_eq!(
            run_with_default(r#"@media (min-width: 500px) {}"#),
            r#"@media(min-width:5rem){}"#
        );
    }

    #[test]
    fn test_margin_shortcuts() {
        assert_eq!(
            run_with_default(r#".a { margin: 0 0 0 100px }"#),
            r#".a{margin:0 0 0 1rem}"#
        );
    }

    #[test]
    fn test_css_variables() {
        assert_eq!(
            run_with_default(r#".a { --a-b: var(--c-d, 88px); }"#),
            r#".a{--a-b:var(--c-d, 0.88rem)}"#
        );
    }

    #[test]
    fn test_prop_blacklist() {
        assert_eq!(
            run(
                r#".a{width:100px;height:100px;}"#,
                Px2RemConfig {
                    prop_black_list: vec!["width".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px;height:1rem}"#
        );
    }

    #[test]
    fn test_prop_whitelist() {
        assert_eq!(
            run(
                r#".a{width:100px;height:100px;}"#,
                Px2RemConfig {
                    prop_white_list: vec!["width".to_string()],
                    prop_black_list: vec![],
                    ..Default::default()
                }
            ),
            r#".a{width:1rem;height:100px}"#
        );
    }

    #[test]
    fn test_select_whitelist() {
        assert_eq!(
            run(
                r#".a{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_white_list: vec!["a".to_string()],
                    selector_black_list: vec![],
                    ..Default::default()
                }
            ),
            r#".a{width:1rem}.b{width:100px}"#
        );
    }

    #[test]
    fn test_select_blacklist() {
        assert_eq!(
            run(
                r#".a{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec!["a".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec!["a".to_string()],
                    ..Default::default()
                }
            ),
            // .ac is matched by "a"
            r#".a{width:100px}.ac{width:100px}.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec!["^a$".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.ac{width:1rem}.b{width:1rem}"#
        );
    }

    fn run_with_default(css_code: &str) -> String {
        run(css_code, Px2RemConfig::default())
    }

    fn run(css_code: &str, config: Px2RemConfig) -> String {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), true);
        let ast = test_utils.ast.css_mut();
        let mut visitor = Px2Rem::new(config);
        ast.ast.visit_mut_with(&mut visitor);
        let code = test_utils.css_ast_to_code();
        println!("{}", code);
        code
    }
}
