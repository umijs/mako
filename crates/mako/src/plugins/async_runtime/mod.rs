use std::sync::Arc;

use crate::compiler::Context;
use crate::plugin::Plugin;

pub struct AsyncRuntimePlugin {}

impl Plugin for AsyncRuntimePlugin {
    fn name(&self) -> &str {
        "async_runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        if context
            .module_graph
            .read()
            .unwrap()
            .modules()
            .iter()
            .any(|module| module.info.as_ref().unwrap().is_async)
        {
            Ok(vec![include_str!("async_runtime.js").to_string()])
        } else {
            Ok(vec![])
        }
    }
}
