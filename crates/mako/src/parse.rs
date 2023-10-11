use std::sync::Arc;

use anyhow::Result;
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
    mako_core::mako_profile_function!(&request.path);
    debug!("parse {:?}", request);
    let ast = context
        .plugin_driver
        .parse(&PluginParseParam { request, content }, context)?
        .unwrap();
    Ok(ast)
}
