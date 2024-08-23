use cached::proc_macro::cached;
use regex::Regex;
use swc_core::css::ast::{
    self as swc_css_ast, AttributeSelector, Combinator, CombinatorValue, ComplexSelector,
    ComplexSelectorChildren, CompoundSelector, Ident, Length, MediaQueryList, Number,
    SubclassSelector, Token, TypeSelector, UnknownDimension,
};
use swc_core::css::visit::{VisitMut, VisitMutWith};

use crate::config::Px2RemConfig;

pub(crate) fn default_root() -> f64 {
    100.0
}

pub(crate) fn default_unit() -> String {
    "px".to_string()
}

pub struct Px2Rem {
    pub config: Px2RemConfig,
    current_decl: Option<String>,
    current_selectors: Vec<String>,
    selector_blacklist: Vec<Regex>,
    selector_whitelist: Vec<Regex>,
    selector_doublelist: Vec<Regex>,
}

impl Px2Rem {
    pub fn new(config: Px2RemConfig) -> Self {
        let selector_blacklist = parse_patterns(&config.selector_blacklist);
        let selector_whitelist = parse_patterns(&config.selector_whitelist);
        let selector_doublelist = parse_patterns(&config.selector_doublelist);
        Self {
            config,
            current_decl: None,
            current_selectors: vec![],
            selector_blacklist,
            selector_whitelist,
            selector_doublelist,
        }
    }

    fn is_any_in_doublelist(&self) -> bool {
        let is_any_in_doublelist = self.current_selectors.iter().any(|selector| {
            self.selector_doublelist
                .iter()
                .any(|regx| regx.is_match(selector))
        });
        is_any_in_doublelist
    }

    fn should_transform(&self, val: f64) -> bool {
        if val.abs() < self.config.min_pixel_value {
            return false;
        }
        let is_prop_valid = if let Some(decl) = &self.current_decl {
            let is_whitelist_empty = self.config.prop_whitelist.is_empty();
            let is_in_whitelist = self.config.prop_whitelist.contains(decl);
            let is_in_blacklist = self.config.prop_blacklist.contains(decl);
            (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist
        } else {
            true
        };
        let is_selector_valid = {
            if self.current_selectors.is_empty() {
                return true;
            }
            let is_whitelist_empty = self.config.selector_whitelist.is_empty();
            let is_all_in_whitelist = self.current_selectors.iter().all(|selector| {
                self.selector_whitelist
                    .iter()
                    .any(|regx| regx.is_match(selector))
            });

            let is_any_in_blacklist = self.current_selectors.iter().any(|selector| {
                self.selector_blacklist
                    .iter()
                    .any(|regx| regx.is_match(selector))
            });
            (is_whitelist_empty || is_all_in_whitelist || self.is_any_in_doublelist())
                && !is_any_in_blacklist
        };
        is_prop_valid && is_selector_valid
    }

    fn transform_declaration_unit(&mut self, value: &mut Number, unit: &mut Ident) {
        value.value /= self.config.root;
        if self.is_any_in_doublelist() {
            value.value *= 2.0;
        }
        value.raw = None;
        unit.value = "rem".into();
    }
}

impl VisitMut for Px2Rem {
    fn visit_mut_complex_selector(&mut self, n: &mut ComplexSelector) {
        let selector = parse_complex_selector(n);
        self.current_selectors.push(selector);
        n.visit_mut_children_with(self);
    }

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

    fn visit_mut_length(&mut self, n: &mut Length) {
        if n.unit.value == self.config.unit && self.should_transform(n.value.value) {
            self.transform_declaration_unit(&mut n.value, &mut n.unit);
        }
        n.visit_mut_children_with(self);
    }

    // custom unit use as unknown_dimension
    // e.g.
    // .a { width: 100rpx; }
    // the `rpx` is a unknown_dimension
    fn visit_mut_unknown_dimension(&mut self, n: &mut UnknownDimension) {
        if n.unit.value == self.config.unit && self.should_transform(n.value.value) {
            self.transform_declaration_unit(&mut n.value, &mut n.unit);
        }
        n.visit_mut_children_with(self);
    }

    fn visit_mut_media_query_list(&mut self, n: &mut MediaQueryList) {
        if self.config.media_query {
            n.visit_mut_children_with(self);
        }
    }

    fn visit_mut_qualified_rule(&mut self, n: &mut swc_css_ast::QualifiedRule) {
        self.current_selectors = vec![];
        n.visit_mut_children_with(self);
    }

    // css variables use as token
    // e.g.
    // .a { --a-b: var(--c-d, 88px); }
    fn visit_mut_token(&mut self, t: &mut Token) {
        if let Token::Dimension(dimension) = t {
            if dimension.unit == self.config.unit && self.should_transform(dimension.value) {
                let mut rem_val = dimension.value / self.config.root;
                if self.is_any_in_doublelist() {
                    rem_val *= 2.0;
                }
                dimension.raw_value = rem_val.to_string().into();
                dimension.value = rem_val;
                dimension.raw_unit = "rem".into();
                dimension.unit = "rem".into();
            }
        }
        t.visit_mut_children_with(self);
    }
}

#[cached(key = "String", convert = r#"{ patterns.join(",") }"#)]
fn parse_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|pattern| {
            let pattern = if pattern.starts_with('/') && pattern.ends_with('/') {
                pattern[1..pattern.len() - 1].to_string()
            } else {
                format!("^{}$", pattern)
            };
            Regex::new(pattern.as_str()).unwrap()
        })
        .collect()
}

fn parse_combinator(combinator: &Combinator) -> String {
    match combinator.value {
        CombinatorValue::Descendant => " ".to_string(),
        CombinatorValue::Child => ">".to_string(),
        CombinatorValue::NextSibling => "+".to_string(),
        CombinatorValue::LaterSibling => "~".to_string(),
        CombinatorValue::Column => "||".to_string(),
    }
}

fn parse_compound_selector(selector: &CompoundSelector) -> String {
    let mut result = String::new();
    // TODO: support selector.nesting_selector
    if let Some(type_selector) = &selector.type_selector {
        let type_selector = type_selector.as_ref();
        match type_selector {
            TypeSelector::TagName(tag_name_selector) => {
                result.push_str(tag_name_selector.name.value.value.as_ref());
            }
            TypeSelector::Universal(_) => {
                result.push('*');
            }
        }
    }
    for subclass_selector in &selector.subclass_selectors {
        match subclass_selector {
            SubclassSelector::Id(id) => {
                result.push_str(&format!("#{}", id.text.value));
            }
            SubclassSelector::Class(class) => {
                result.push_str(&format!(".{}", class.text.value));
            }
            SubclassSelector::Attribute(attr) => {
                result.push_str(parse_attribute(attr).as_str());
            }
            SubclassSelector::PseudoClass(pseudo) => {
                result.push_str(format!(":{}", pseudo.name.value).as_str());
            }
            _ => {
                // TODO: support more subclass selectors
            }
        }
    }
    result
}

fn parse_attribute(attr: &AttributeSelector) -> String {
    let mut res_str = String::new();
    let AttributeSelector {
        name,
        matcher,
        value,
        ..
    } = attr;
    let val_str = if let Some(val_str) = value.as_ref() {
        if let Some(val_str) = val_str.as_str() {
            val_str.value.to_string()
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };
    let matcher = if let Some(x) = matcher.as_ref() {
        x.value.to_string()
    } else {
        "".to_string()
    };
    res_str.push_str(&format!("[{}{}{}]", name.value.value, matcher, val_str));
    res_str
}

fn parse_complex_selector(selector: &ComplexSelector) -> String {
    let mut result = String::new();
    for child in &selector.children {
        match child {
            ComplexSelectorChildren::CompoundSelector(compound_selector) => {
                result.push_str(parse_compound_selector(compound_selector).as_str());
            }
            ComplexSelectorChildren::Combinator(combinator, ..) => {
                result.push_str(parse_combinator(combinator).as_str());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use swc_core::css::visit::VisitMutWith;

    use super::Px2Rem;
    use crate::ast::tests::TestUtils;
    use crate::config::Px2RemConfig;

    #[test]
    fn test_normal() {
        assert_eq!(
            run_with_default(r#".a{width:100px;height:200px;}"#),
            r#".a{width:1rem;height:2rem}"#
        );
        assert_eq!(
            run_with_min_pixel_value(r#".a{width:100px;height:200px;}"#, 101.0),
            r#".a{width:100px;height:2rem}"#
        );
        assert_eq!(
            run_with_min_pixel_value(r#".a{width:100px;height:-200px;}"#, 101.0),
            r#".a{width:100px;height:-2rem}"#
        );
    }

    #[test]
    fn test_attribute_selector() {
        assert_eq!(
            run_with_default(r#".a[b]{width:100px;}"#),
            r#".a[b]{width:1rem}"#
        );
        assert_eq!(
            run_with_default(r#".a[b=c]{width:100px;}"#),
            r#".a[b=c]{width:1rem}"#
        );
    }

    #[test]
    fn test_media_query_off() {
        assert_eq!(
            run_with_default(
                r#"@media (min-width: 500px) {
                h1{ width: 100px; }
            }"#
            ),
            r#"@media(min-width:500px){h1{width:1rem}}"#
        );
    }
    #[test]

    fn test_media_query_on() {
        assert_eq!(
            run(
                r#"@media (min-width: 500px) {
                h1{ width: 100px; }
            }"#,
                Px2RemConfig {
                    media_query: true,
                    ..Default::default()
                }
            ),
            r#"@media(min-width:5rem){h1{width:1rem}}"#
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
        assert_eq!(
            run_with_min_pixel_value(r#".a { --a-b: var(--c-d, 88px); }"#, 100.0),
            r#".a{--a-b:var(--c-d, 88px)}"#
        );
        assert_eq!(
            run_with_min_pixel_value(r#".a { --a-b: var(--c-d, 88px); }"#, 88.0),
            r#".a{--a-b:var(--c-d, 0.88rem)}"#
        );
    }

    #[test]
    fn test_prop_blacklist() {
        assert_eq!(
            run(
                r#".a{width:100px;height:100px;}"#,
                Px2RemConfig {
                    prop_blacklist: vec!["width".to_string()],
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
                    prop_whitelist: vec!["width".to_string()],
                    prop_blacklist: vec![],
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
                    selector_whitelist: vec![".a".to_string()],
                    selector_blacklist: vec![],
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
                    selector_blacklist: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.b{width:1rem}"#
        );
    }

    #[test]
    fn test_selector_blacklist_exact_match() {
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            // .ac should not be matched by .a
            r#".a{width:100px}.ac{width:1rem}.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.ac{width:100px;}.b{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec!["/^.a$/".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:100px}.ac{width:1rem}.b{width:1rem}"#
        );
    }

    #[test]
    fn test_selector_blacklist_id() {
        assert_eq!(
            run(
                r#"#a{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec!["#a".to_string()],
                    ..Default::default()
                }
            ),
            r#"#a{width:100px}"#
        );
    }

    #[test]
    fn test_selector_blacklist_tagname() {
        assert_eq!(
            run(
                r#"div{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec!["div".to_string()],
                    ..Default::default()
                }
            ),
            r#"div{width:100px}"#
        );
    }

    #[test]
    fn test_selector_blacklist_unique() {
        assert_eq!(
            run(
                r#"div *{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec!["/div */".to_string()],
                    ..Default::default()
                }
            ),
            r#"div *{width:100px}"#
        );
    }

    #[test]
    fn test_selector_blacklist_multiple_classes() {
        assert_eq!(
            run(
                r#".a.b{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec![".a.b".to_string()],
                    ..Default::default()
                }
            ),
            r#".a.b{width:100px}"#
        );
    }

    #[test]
    fn test_selector_blacklist_child() {
        assert_eq!(
            run(
                r#".a > .b{width:100px;}"#,
                Px2RemConfig {
                    // TODO: handle .a > .b (with space in between)
                    selector_blacklist: vec![".a>.b".to_string()],
                    ..Default::default()
                }
            ),
            r#".a>.b{width:100px}"#
        );
    }
    #[test]
    fn test_selector_attribute_selector_black() {
        assert_eq!(
            run(
                r#"[class*="button"]{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec!["/[class*=\"button\"]/".to_string()],
                    ..Default::default()
                }
            ),
            r#"[class*="button"]{width:100px}"#
        );
    }

    #[test]
    fn test_selector_attribute_selector_white() {
        assert_eq!(
            run(
                r#"[class*="button"]{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec!["/[class*=\"button\"]/".to_string()],
                    ..Default::default()
                }
            ),
            r#"[class*="button"]{width:1rem}"#
        );
    }

    #[test]
    fn test_attribute() {
        assert_eq!(
            run(
                r#"[class*="button"]{width:100px;}"#,
                Px2RemConfig {
                    ..Default::default()
                }
            ),
            r#"[class*="button"]{width:1rem}"#
        );
    }

    #[test]
    fn test_class_pseudo() {
        assert_eq!(
            run(
                r#".jj:before,.jj:after{width:100px;}"#,
                Px2RemConfig {
                    ..Default::default()
                }
            ),
            r#".jj:before,.jj:after{width:1rem}"#
        );
    }

    #[test]
    fn test_class_pseudo_select_black() {
        assert_eq!(
            run(
                r#".jj:before,.jj:after{width:100px;}"#,
                Px2RemConfig {
                    selector_blacklist: vec![".jj:after".to_string()],
                    ..Default::default()
                }
            ),
            r#".jj:before,.jj:after{width:100px}"#
        );
    }

    #[test]
    fn test_class_pseudo_select_white() {
        assert_eq!(
            run(
                r#".jj:before,.jj:after{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec![".jj:after".to_string()],
                    ..Default::default()
                }
            ),
            r#".jj:before,.jj:after{width:100px}"#
        );
    }

    #[test]
    fn test_multi_selectors_whitelist() {
        assert_eq!(
            run(
                r#".a,.b{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec![],
                    selector_blacklist: vec![],
                    ..Default::default()
                }
            ),
            r#".a,.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a,.b{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec![".a".to_string()],
                    selector_blacklist: vec![],
                    ..Default::default()
                }
            ),
            r#".a,.b{width:100px}"#
        );
        assert_eq!(
            run(
                r#".a,.b{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec![".a".to_string(), ".b".to_string()],
                    selector_blacklist: vec![],
                    ..Default::default()
                }
            ),
            r#".a,.b{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a,.b{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec![],
                    selector_blacklist: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            r#".a,.b{width:100px}"#
        );
    }

    #[test]
    fn test_multi_selector_doublelist() {
        assert_eq!(
            run(
                r#".a{width:100px;}"#,
                Px2RemConfig {
                    selector_doublelist: vec![],
                    ..Default::default()
                }
            ),
            r#".a{width:1rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}"#,
                Px2RemConfig {
                    selector_doublelist: vec![".a".to_string()],
                    ..Default::default()
                }
            ),
            r#".a{width:2rem}"#
        );
        assert_eq!(
            run(
                r#".a-x{width:100px;}"#,
                Px2RemConfig {
                    selector_doublelist: vec!["/^.a-/".to_string()],
                    ..Default::default()
                }
            ),
            r#".a-x{width:2rem}"#
        );
        assert_eq!(
            run(
                r#":root{width:100px;}"#,
                Px2RemConfig {
                    selector_doublelist: vec!["^:root".to_string()],
                    ..Default::default()
                }
            ),
            r#":root{width:2rem}"#
        );
        assert_eq!(
            run(
                r#".a{width:100px;}.b{width:100px;}.b.a{width:100px;}"#,
                Px2RemConfig {
                    selector_whitelist: vec!["/.a/".to_string()],
                    selector_blacklist: vec![],
                    ..Default::default()
                }
            ),
            r#".a{width:1rem}.b{width:100px}.b.a{width:1rem}"#
        );
    }

    #[test]
    fn test_special_custom_unit() {
        assert_eq!(
            run(
                r#".a{width:100rpx;height:40px;}"#,
                Px2RemConfig {
                    unit: "rpx".to_string(),
                    ..Default::default()
                }
            ),
            r#".a{width:1rem;height:40px}"#
        );
    }

    fn run_with_default(css_code: &str) -> String {
        run(css_code, Px2RemConfig::default())
    }

    fn run_with_min_pixel_value(css_code: &str, min_pixel_value: f64) -> String {
        run(
            css_code,
            Px2RemConfig {
                min_pixel_value,
                ..Default::default()
            },
        )
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
