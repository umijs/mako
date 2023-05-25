use swc_common::sync::Lrc;
use swc_common::SourceMap;
use tracing::debug;

use crate::ast::{build_css_ast, build_js_ast};
use crate::load::Asset;
use crate::{load::Content, module::ModuleAst};

pub fn parse(content: &Content, path: &str) -> (ModuleAst, Lrc<SourceMap>) {
    debug!("parse {}", path);
    match content {
        Content::Js(content) => parse_js(content, path),
        Content::Css(content) => parse_css(content, path),
        Content::Assets(asset) => parse_asset(asset, path),
    }
}

fn parse_js(content: &str, path: &str) -> (ModuleAst, Lrc<SourceMap>) {
    let (cm, ast) = build_js_ast(path, content);
    (ModuleAst::Script(ast), cm)
}

fn parse_css(content: &str, path: &str) -> (ModuleAst, Lrc<SourceMap>) {
    let (cm, ast) = build_css_ast(path, content);
    (ModuleAst::Css(ast), cm)
}

fn parse_asset(asset: &Asset, path: &str) -> (ModuleAst, Lrc<SourceMap>) {
    let (cm, ast) = build_js_ast(path, &asset.content);
    (ModuleAst::Script(ast), cm)
}
