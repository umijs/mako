use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;

pub struct ManifestPlugin {}

impl Plugin for ManifestPlugin {
    fn name(&self) -> &str {
        "manifest"
    }

    fn build_success(&self, stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        if context.config.manifest {
            println!("stats: {:?}", stats);
        }
        Ok(None)
    }
}
