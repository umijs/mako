use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::load::{content_hash, Asset, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct WASMPlugin {}

impl Plugin for WASMPlugin {
    fn name(&self) -> &str {
        "wasm"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        // wasm don't need to support base64
        if matches!(param.ext_name.as_str(), "wasm") {
            let final_file_name =
                content_hash(param.path.as_str())? + "." + param.ext_name.as_str();
            context.emit_assets(param.path.clone(), final_file_name.clone());
            return Ok(Some(Content::Assets(Asset {
                path: param.path.clone(),
                content: format!(
                    "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                    final_file_name
                ),
            })));
        }
        Ok(None)
    }
}
