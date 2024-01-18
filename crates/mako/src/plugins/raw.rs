use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct RawPlugin {}

impl Plugin for RawPlugin {
    fn name(&self) -> &str {
        "raw"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if param.task.request.query.iter().any(|item| item.0 == "raw") {
            let file_content = read_content(param.task.request.path.as_str())?;
            let json_string = serde_json::to_string(&file_content)?;

            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                json_string
            ))));
        }
        Ok(None)
    }
}
