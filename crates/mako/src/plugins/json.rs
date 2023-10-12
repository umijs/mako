use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct JSONPlugin {}

impl Plugin for JSONPlugin {
    fn name(&self) -> &str {
        "json"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        // TODO: json5 应该没这么简单
        if matches!(param.ext_name.as_str(), "json" | "json5") {
            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                read_content(param.path.as_str())?
            ))));
        }
        Ok(None)
    }
}
