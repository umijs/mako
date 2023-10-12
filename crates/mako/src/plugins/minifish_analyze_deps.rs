use mako_core::anyhow::Result;

use crate::plugin::{Plugin, PluginDepAnalyzeParam};

pub struct MinifishDepsAnalyze {}

impl Plugin for MinifishDepsAnalyze {
    fn name(&self) -> &str {
        "mini_deps_analyze"
    }

    fn analyze_deps(&self, analyze_param: &mut PluginDepAnalyzeParam) -> Result<()> {
        analyze_param
            .deps
            .retain(|dep| !dep.source.ends_with("_minifish_global_provider.js"));

        Ok(())
    }
}
