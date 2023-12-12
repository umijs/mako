use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::tracing::debug;

use crate::compiler::Context;
use crate::load::Content;
use crate::module::ModuleAst;
use crate::plugin::PluginParseParam;
use crate::task::Task;

pub fn parse(content: &Content, task: &Task, context: &Arc<Context>) -> Result<ModuleAst> {
    mako_core::mako_profile_function!(&task.path);
    debug!("parse {:?}", task);
    let ast = context
        .plugin_driver
        .parse(&PluginParseParam { task, content }, context)?
        .unwrap();
    Ok(ast)
}
