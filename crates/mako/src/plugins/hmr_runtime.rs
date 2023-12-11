use std::sync::Arc;

use mako_core::anyhow;

use crate::compiler::Context;
use crate::plugin::Plugin;

pub struct HMRRuntimePlugin {}

impl Plugin for HMRRuntimePlugin {
    fn name(&self) -> &str {
        "hmr_runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        if context.args.watch {
            Ok(vec![include_str!("hmr_runtime/hmr_runtime.js").to_string()])
        } else {
            Ok(vec![])
        }
    }
}
