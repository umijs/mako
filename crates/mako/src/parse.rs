use std::sync::Arc;

use anyhow::Result;
use swc_css_ast::Stylesheet;
use swc_css_visit::VisitMutWith;
use tracing::debug;

use crate::ast::{build_css_ast, build_js_ast};
use crate::build::FileRequest;
use crate::compiler::Context;
use crate::css_modules::{compile_css_modules, generate_code_for_css_modules, is_css_modules_path};
use crate::load::{Asset, Content};
use crate::module::ModuleAst;

pub fn parse(
    content: &Content,
    request: &FileRequest,
    context: &Arc<Context>,
) -> Result<ModuleAst> {
    debug!("parse {:?}", request);
    let ast = match content {
        Content::Js(content) => parse_js(content, request, context)?,
        Content::Css(content) => parse_css(content, request, context)?,
        Content::Assets(asset) => parse_asset(asset, request, context)?,
    };
    Ok(ast)
}

fn parse_js(content: &str, request: &FileRequest, context: &Arc<Context>) -> Result<ModuleAst> {
    let ast = build_js_ast(&request.path, content, context)?;
    Ok(ModuleAst::Script(ast))
}

fn parse_css(content: &str, request: &FileRequest, context: &Arc<Context>) -> Result<ModuleAst> {
    let mut ast = build_css_ast(&request.path, content, context)?;
    let is_modules = request.has_query("modules");
    // parse css module as js
    if is_css_modules_path(&request.path) && !is_modules {
        let code = generate_code_for_css_modules(&request.path, &mut ast);
        let js_ast = build_js_ast(&request.path, &code, context)?;
        Ok(ModuleAst::Script(js_ast))
    } else {
        // TODO: move to transform step
        // compile css compat
        compile_css_compat(&mut ast);
        // for mako css module, compile it and parse it as css
        if is_modules {
            compile_css_modules(&request.path, &mut ast);
        }
        Ok(ModuleAst::Css(ast))
    }
}

fn parse_asset(asset: &Asset, request: &FileRequest, context: &Arc<Context>) -> Result<ModuleAst> {
    let ast = build_js_ast(&request.path, &asset.content, context)?;
    Ok(ModuleAst::Script(ast))
}

fn compile_css_compat(ast: &mut Stylesheet) {
    ast.visit_mut_with(&mut swc_css_compat::compiler::Compiler::new(
        swc_css_compat::compiler::Config {
            process: swc_css_compat::feature::Features::NESTING,
        },
    ));
}
