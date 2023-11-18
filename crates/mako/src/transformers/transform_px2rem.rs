use std::sync::Arc;

use mako_core::swc_css_ast::{self, Length, Token};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;

pub struct Px2Rem<'a> {
    pub context: &'a Arc<Context>,
    pub path: &'a str,
    pub current_decl: Option<String>,
    // TODO: support selector
    pub current_selector: Option<String>,
}

impl Px2Rem<'_> {
    fn should_transform(&self) -> bool {
        if let Some(current_decl) = &self.current_decl {
            let is_in_whitelist = self.context.config.px2rem_config.prop_white_list.is_empty()
                || self
                    .context
                    .config
                    .px2rem_config
                    .prop_white_list
                    .contains(current_decl);
            let is_in_blacklist = self
                .context
                .config
                .px2rem_config
                .prop_black_list
                .contains(current_decl);
            return is_in_whitelist && !is_in_blacklist;
        }
        true
    }
}

impl VisitMut for Px2Rem<'_> {
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
            n.value.value /= self.context.config.px2rem_config.root;
            n.value.raw = None;
            n.unit.value = "rem".into();
        }
        n.visit_mut_children_with(self);
    }
    fn visit_mut_token(&mut self, t: &mut Token) {
        if let Token::Dimension(dimension) = t {
            dimension.raw_value = (dimension.value / self.context.config.px2rem_config.root)
                .to_string()
                .into();
            dimension.raw_unit = "rem".into();
        }
        t.visit_mut_children_with(self);
    }
}
