use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::load::{content_hash, file_name, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct WASMPlugin {}

impl Plugin for WASMPlugin {
    fn name(&self) -> &str {
        "wasm"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        // wasm don't need to support base64
        if param.task.is_match(vec!["wasm"]) {
            // add.wasm => add.hash.wasm
            let final_file_name = format!(
                "{}.{}.{}",
                file_name(param.task.request.path.as_str()).unwrap(),
                content_hash(param.task.request.path.as_str())?,
                param.task.ext_name.as_ref().unwrap()
            );
            context.emit_assets(param.task.request.path.clone(), final_file_name.clone());
            return Ok(Some(Content::Js(format!(
                "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                final_file_name
            ))));
        }
        Ok(None)
    }
}
