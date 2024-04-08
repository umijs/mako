use std::sync::Arc;

use mako_core::regex::Regex;
use mako_core::swc_css_ast::{self, Length, Token};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};
use swc_core::css::ast::{CompoundSelector, SubclassSelector};

use crate::compiler::Context;

pub(crate) fn default_root() -> f64 {
    100.0
}
pub struct Px2Rem {
    pub context: Arc<Context>,
    pub path: String,
    pub current_decl: Option<String>,
    // TODO: support selector
    pub current_selector: Option<String>,
}

impl Px2Rem {
    pub fn new(current_decl: String, current_selector: String) -> Self {
        Self {
            context: Arc::new(Default::default()),
            path: "".to_string(),
            current_decl: Some(current_decl),
            current_selector: Some(current_selector),
        }
    }
    fn should_transform(&self) -> bool {
        let mut is_in_is_in_whitelist_and_is_in_blacklist = true;
        is_in_is_in_whitelist_and_is_in_blacklist = if let Some(current_decl) = &self.current_decl {
            let px2rem_config = self
                .context
                .config
                .px2rem
                .as_ref()
                .expect("px2rem config should exist");
            let is_in_whitelist = px2rem_config.prop_white_list.is_empty()
                || px2rem_config.prop_white_list.contains(current_decl);

            let is_in_blacklist = px2rem_config.prop_black_list.contains(current_decl);

            is_in_whitelist && !is_in_blacklist
        } else {
            false // 或者选择一个合适的默认值
        };
        let mut is_in_is_select_black = true;
        let selector_black_list = &self
            .context
            .as_ref()
            .config
            .px2rem
            .as_ref()
            .unwrap()
            .selector_black_list;
        for reg_String in selector_black_list {
            let re = Regex::new(reg_String).unwrap();
            is_in_is_select_black =
                !re.is_match(self.current_selector.as_ref().unwrap_or(&String::from("")));
            if !is_in_is_select_black {
                break;
            }
        }
        is_in_is_in_whitelist_and_is_in_blacklist && is_in_is_select_black
    }
}

impl VisitMut for Px2Rem {
    // 应该是这里决定的? 传入不同的类型转换为不同的
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
        println!("CompoundSelectorLHL=={:?}", n);
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
        // self.current_selector = match &n { CompoundSelector{..}=>Some() }
        n.visit_mut_children_with(self);
    }
    fn visit_mut_length(&mut self, n: &mut Length) {
        if n.unit.value.to_string() == "px" && self.should_transform() {
            n.value.value /= self.context.config.px2rem.as_ref().unwrap().root;
            n.value.raw = None;
            n.unit.value = "rem".into();
        }
        n.visit_mut_children_with(self);
    }
    fn visit_mut_token(&mut self, t: &mut Token) {
        if let Token::Dimension(dimension) = t {
            if dimension.unit.to_string() == "px" && self.should_transform() {
                let rem_val = dimension.value / self.context.config.px2rem.as_ref().unwrap().root;
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

    use crate::ast_2::tests::TestUtils;

    #[test]
    fn test_keep_none_relative() {
        run(r#"@import "./foo.css";
/* @import url("https://fonts.googleapis.com/css?family=Open+Sans"); */

h1 {
    background: url('./assets/person.svg') 200px no-repeat;
    background-size: 20px 20px;
    font-family: 'Open Sans';
}
.remkkk{
    width: 100px;
}
div{
    height: 300px;
}
"#);
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
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string());
        let ast = test_utils.ast.css_mut();
        println!("ast==={:?}", ast);
        String::from("")
        // let mut visitor = super::CSSImports {};
        // ast.ast.visit_mut_with(&mut visitor);
        // test_utils.css_ast_to_code()
    }
}
