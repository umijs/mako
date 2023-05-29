use std::sync::Arc;
use tracing::debug;

use crate::ast::{build_css_ast, build_js_ast};
use crate::compiler::Context;
use crate::load::Asset;
use crate::{load::Content, module::ModuleAst};

pub fn parse(content: &Content, path: &str, context: &Arc<Context>) -> ModuleAst {
    debug!("parse {}", path);
    match content {
        Content::Js(content) => parse_js(content, path, context),
        Content::Css(content) => parse_css(content, path, context),
        Content::Assets(asset) => parse_asset(asset, path, context),
    }
}

fn parse_js(content: &str, path: &str, context: &Arc<Context>) -> ModuleAst {
    let ast = build_js_ast(path, content, context);
    ModuleAst::Script(ast)
}

fn parse_css(content: &str, path: &str, context: &Arc<Context>) -> ModuleAst {
    let ast = build_css_ast(path, content, context);
    ModuleAst::Css(ast)
}

fn parse_asset(asset: &Asset, path: &str, context: &Arc<Context>) -> ModuleAst {
    let ast = build_js_ast(path, &asset.content, context);
    ModuleAst::Script(ast)
}
