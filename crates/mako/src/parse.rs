use std::sync::Arc;

use anyhow::Result;
use tracing::debug;

use crate::ast::{build_css_ast, build_js_ast};
use crate::compiler::Context;
use crate::css_modules::{
    compile_css_modules, generate_code_for_css_modules, is_css_modules_path, is_mako_css_modules,
    MAKO_CSS_MODULES_SUFFIX,
};
use crate::load::{Asset, Content};
use crate::module::ModuleAst;

pub fn parse(content: &Content, path: &str, context: &Arc<Context>) -> Result<ModuleAst> {
    debug!("parse {}", path);
    let ast = match content {
        Content::Js(content) => parse_js(content, path, context)?,
        Content::Css(content) => parse_css(content, path, context)?,
        Content::Assets(asset) => parse_asset(asset, path, context)?,
    };
    Ok(ast)
}

fn parse_js(content: &str, path: &str, context: &Arc<Context>) -> Result<ModuleAst> {
    let ast = build_js_ast(path, content, context)?;
    Ok(ModuleAst::Script(ast))
}

fn parse_css(content: &str, path: &str, context: &Arc<Context>) -> Result<ModuleAst> {
    let mut ast = build_css_ast(path, content, context)?;
    // parse css module as js
    if is_css_modules_path(path) {
        let code = generate_code_for_css_modules(path, &mut ast);
        let js_ast = build_js_ast(path, &code, context)?;
        Ok(ModuleAst::Script(js_ast))
    } else {
        // for mako css module, compile it and parse it as css
        if is_mako_css_modules(path) {
            // should remove the suffix to generate the same hash
            compile_css_modules(path.trim_end_matches(MAKO_CSS_MODULES_SUFFIX), &mut ast);
        }
        Ok(ModuleAst::Css(ast))
    }
}

fn parse_asset(asset: &Asset, path: &str, context: &Arc<Context>) -> Result<ModuleAst> {
    let ast = build_js_ast(path, &asset.content, context)?;
    Ok(ModuleAst::Script(ast))
}
