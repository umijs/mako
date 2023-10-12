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
        if matches!(param.ext_name.as_str(), "wasm") {
            // add.wasm => add.hash.wasm
            let final_file_name = format!(
                "{}.{}.{}",
                file_name(param.path.as_str()).unwrap(),
                content_hash(param.path.as_str())?,
                param.ext_name.as_str()
            );
            context.emit_assets(param.path.clone(), final_file_name.clone());
            return Ok(Some(Content::Js(format!(
                "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                final_file_name
            ))));
        }
        Ok(None)
    }
}
