use std::sync::Arc;

use anyhow;

use crate::compiler::Context;
use crate::plugin::Plugin;

pub struct WasmRuntimePlugin {}

impl Plugin for WasmRuntimePlugin {
    fn name(&self) -> &str {
        "wasm_runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        if context
            .assets_info
            .lock()
            .unwrap()
            .values()
            .any(|info| info.ends_with(".wasm"))
        {
            Ok(vec![include_str!("wasm_runtime.js").to_string()])
        } else {
            Ok(vec![])
        }
    }
}
