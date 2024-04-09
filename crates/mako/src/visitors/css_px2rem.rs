use mako_core::swc_css_ast::{self, Length, Token};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};

use crate::config::Px2RemConfig;

pub(crate) fn default_root() -> f64 {
    100.0
}

pub struct Px2Rem {
    pub config: Px2RemConfig,
    pub current_decl: Option<String>,
    // TODO: support selector
    pub current_selector: Option<String>,
}

impl Px2Rem {
    fn should_transform(&self) -> bool {
        if let Some(current_decl) = &self.current_decl {
            let is_whitelist_empty = self.config.prop_white_list.is_empty();
            let is_in_whitelist = self.config.prop_white_list.contains(current_decl);
            let is_in_blacklist = self.config.prop_black_list.contains(current_decl);
            return (is_whitelist_empty || is_in_whitelist) && !is_in_blacklist;
        }
        true
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
    fn test_blacklist() {
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
    fn test_whitelist() {
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

    fn run_with_default(css_code: &str) -> String {
        run(css_code, Px2RemConfig::default())
    }

    fn run(css_code: &str, config: Px2RemConfig) -> String {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), true);
        let ast = test_utils.ast.css_mut();
        let mut visitor = super::Px2Rem {
            config,
            current_decl: None,
            current_selector: None,
        };
        ast.ast.visit_mut_with(&mut visitor);
        let code = test_utils.css_ast_to_code();
        println!("{}", code);
        code
    }
}
