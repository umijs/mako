use std::sync::Arc;

use anyhow::Result;
use swc_css_ast::Stylesheet;
use swc_css_visit::VisitMutWith;
use tracing::debug;

use crate::build::FileRequest;
use crate::compiler::Context;
use crate::load::Content;
use crate::module::ModuleAst;
use crate::plugin::PluginParseParam;

pub fn parse(
    content: &Content,
    request: &FileRequest,
    context: &Arc<Context>,
) -> Result<ModuleAst> {
    debug!("parse {:?}", request);
    let ast = context
        .plugin_driver
        .parse(&PluginParseParam { request, content }, context)?
        .unwrap();
    Ok(ast)
}

pub fn compile_css_compat(ast: &mut Stylesheet) {
    ast.visit_mut_with(&mut swc_css_compat::compiler::Compiler::new(
        swc_css_compat::compiler::Config {
            process: swc_css_compat::feature::Features::NESTING,
        },
    ));
}
