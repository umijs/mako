use anyhow::Result;

use crate::plugin::{Plugin, PluginDepAnalyzeParam};

pub struct DepsAnalyze {}

impl Plugin for DepsAnalyze {
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
