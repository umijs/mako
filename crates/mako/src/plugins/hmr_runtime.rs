use std::sync::Arc;

use crate::compiler::Context;
use crate::config::Mode;
use crate::plugin::Plugin;

pub struct HMRRuntimePlugin {}

impl Plugin for HMRRuntimePlugin {
    fn name(&self) -> &str {
        "hmr_runtime"
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        if _context.config.hmr && _context.config.mode == Mode::Development {
            Ok(vec![include_str!("hmr_runtime/hmr_runtime.js").to_string()])
        } else {
            Ok(vec![])
        }
    }
}
