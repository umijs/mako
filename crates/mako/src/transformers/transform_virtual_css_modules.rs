use std::sync::Arc;

use mako_core::lazy_static::lazy_static;
use mako_core::regex::Regex;
use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::{ImportDecl, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;

pub struct VirtualCSSModules<'a> {
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
}

lazy_static! {
    static ref CSS_MODULES_PATH_REGEX: Regex = Regex::new(r#"\.module\.(css|less)$"#).unwrap();
    static ref CSS_PATH_REGEX: Regex = Regex::new(r#"\.(css|less)$"#).unwrap();
}

fn is_css_modules_path(path: &str) -> bool {
    CSS_MODULES_PATH_REGEX.is_match(path)
}

pub fn is_css_path(path: &str) -> bool {
    CSS_PATH_REGEX.is_match(path)
}

impl VisitMut for VirtualCSSModules<'_> {
    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        if is_css_modules_path(&import_decl.src.value)
            || (self.context.config.auto_css_modules
                && is_css_path(&import_decl.src.value)
                && !&import_decl.specifiers.is_empty())
        {
            self.replace_source(&mut import_decl.src);
        }
        import_decl.visit_mut_children_with(self);
    }
}

impl VirtualCSSModules<'_> {
    fn replace_source(&mut self, source: &mut Str) {
        let to_replace = format!("{}?asmodule", &source.value.to_string());
        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}
