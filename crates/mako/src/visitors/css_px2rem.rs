use mako_core::regex::Regex;
use mako_core::swc_css_ast::{self, Length, Token};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};
use swc_core::css::ast::{Combinator, ComplexSelector, CompoundSelector, SubclassSelector};

use crate::config::Px2RemConfig;

pub(crate) fn default_root() -> f64 {
    100.0
}

pub struct Px2Rem {
    pub config: Px2RemConfig,
    current_decl: Option<String>,
    current_selector: Option<String>,
    select_black_regx: Vec<Regex>,
    select_white_regx: Vec<Regex>,
}

fn match_str_or_regex_based_on_chars(pattern: &str, input: &str, regx_arr: &[Regex]) -> bool {
    if pattern == input {
        // 第一个参数和第二个参数全等
        true
    } else if pattern.contains('*')
        || pattern.contains('\\')
        || pattern.contains('(')
        || pattern.contains(')')
        || pattern.contains('^')
        || pattern.contains('$')
    {
        // 参数包含特定的正则表达式字符，尝试将第一个参数作为正则表达式
        regx_arr.iter().any(|regx| regx.is_match(input))
    } else {
        // 如果不包含特定字符且不全等，返回 false
        false
    }
}

impl Px2Rem {
    pub fn new(config: Px2RemConfig) -> Self {
        let mut select_black_regx = vec![];
        let mut select_white_regx = vec![];
        if !config.selector_black_list.is_empty() {
            select_black_regx = config
                .selector_black_list
                .iter()
                .filter_map(|pattern| Regex::new(pattern).ok()) // 这里 `ok()` 会将 `Result` 转换为 `Option`
                .collect();
            select_white_regx = config
                .selector_white_list
                .iter()
                .filter_map(|pattern| Regex::new(pattern).ok()) // 这里 `ok()` 会将 `Result` 转换为 `Option`
                .collect();
        }
        Self {
            config,
            current_decl: None,
            current_selector: None,
            select_black_regx,
            select_white_regx,
        }
    }

    fn should_transform(&self) -> bool {
        let is_prop_valid = if let Some(decl) = &self.current_decl {
            let is_whitelist_empty = self.config.prop_white_list.is_empty();
            let is_in_whitelist = self.config.prop_white_list.contains(decl);
            let is_in_blacklist = self.config.prop_black_list.contains(decl);
            (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist
        } else {
            true
        };
        let is_selector_valid = if let Some(selector) = &self.current_selector {
            let is_whitelist_empty = self.config.selector_white_list.is_empty();
            // 判断是否是字符串还是正则匹配

            let is_in_whitelist = self.config.selector_white_list.iter().any(|pattern| {
                match_str_or_regex_based_on_chars(pattern, selector, &self.select_white_regx)
            });

            let is_in_blacklist = self.config.selector_black_list.iter().any(|pattern| {
                // TODO: should have performance issues, need benchmark
                match_str_or_regex_based_on_chars(pattern, selector, &self.select_black_regx)
            });

            (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist
        } else {
            true
        };
        is_prop_valid && is_selector_valid
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

    fn visit_mut_complex_selector(&mut self, n: &mut ComplexSelector) {
        // 判断是否有值
        if let Some(ref mut currselect) = self.current_selector {
            currselect.push(",".parse().unwrap())
        } else {
            self.current_selector = None
        }

        n.visit_mut_children_with(self);
    }
    fn visit_mut_combinator(&mut self, n: &mut Combinator) {
        let value = n.value;

        if let Some(ref mut current_sele) = self.current_selector {
            current_sele.push(value.to_string().parse().unwrap());
        }

        n.visit_mut_children_with(self);
    }
    fn visit_mut_compound_selector(&mut self, n: &mut CompoundSelector) {
        let mut cur_select = None;
        if !n.subclass_selectors.is_empty() {
            // 假设我们只关心第一个 subclass_selector，因此获取第一个元素
            // 使用匹配来处理不同类型的 SubclassSelector

            //累加

            match n.subclass_selectors.first().unwrap() {
                SubclassSelector::Class(class_selector) => {
                    cur_select = Some(format!("{}{}", ".", class_selector.text.value.clone()));

                    Some(class_selector.text.value.clone().to_string())
                }
                // ... 处理 SubclassSelector 的其他变体 ...
                _ => None,
            };
        } else if let Some(type_selector) = &n.type_selector {
            if let Some(name) = type_selector.clone().tag_name() {
                cur_select = Some(name.name.value.value.to_string());
            }
        } else {
            self.current_selector = None;
        }
        if let Some(ref mut cur_value) = self.current_selector {
            cur_value.push_str(&cur_select.unwrap_or("".to_string()));
        } else {
            self.current_selector = cur_select;
        }

        n.visit_mut_children_with(self);
    }

    fn visit_mut_length(&mut self, n: &mut Length) {
        if n.unit.value.to_string() == "px" && self.should_transform() {
            n.value.value /= self.config.root;
            n.value.raw = None;
            n.unit.value = "rem".into();
        }
        self.current_selector = None;
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
    fn test_selector_whitelist() {
        assert_eq!(
            run(
                r#".a{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_white_list: vec![".a".to_string()],
                    selector_black_list: vec![],
                    ..Default::default()
                }
            ),
            r#".a{width:1rem}.b{width:100px}"#
        );
    }

    #[test]
    fn test_selector_blacklist() {
        assert_eq!(
            run(
                r#".a{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            // .ac is matched by "a"
            r#".a{width:100px}.ac{width:1rem}.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_black_list: vec!["^.a$".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.ac{width:1rem}.b{width:1rem}"#
        );
    }

    // TODO: FIXME
    // 如果有多个 selector，应该「全满足 whitelist」且「全不满足 blacklist」时才做 transform
    #[test]
    #[ignore]
    fn test_multi_selectors_whitelist() {
        assert_eq!(
            run(
                r#".a .h,.b,.c>.g .kk,h1 div{width:100px;}"#,
                Px2RemConfig {
                    selector_white_list: vec![".a .h,.b,.c>.g .kk,h1 div".to_string()],
                    selector_black_list: vec![".a .h,.b,.c>.g .kk,h1 ".to_string()],
                    ..Default::default()
                }
            ),
            r#".a .h,.b,.c>.g .kk,h1 div{width:1rem}"#
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
