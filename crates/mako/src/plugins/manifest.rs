use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use anyhow::Result;
use serde_json;

use crate::compiler::Context;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;

pub struct ManifestPlugin {}

impl Plugin for ManifestPlugin {
    fn name(&self) -> &str {
        "manifest"
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        if context.config.manifest {
            let assets = &context.stats_info.lock().unwrap().assets;
            let mut manifest: BTreeMap<String, String> = BTreeMap::new();
            let file_name = context
                .config
                .manifest_config
                .get("file_name")
                .unwrap()
                .clone();
            let mut base_path = context
                .config
                .manifest_config
                .get("base_path")
                .unwrap()
                .clone();

            if !base_path.is_empty() && !base_path.ends_with('/') {
                base_path.push('/');
            }

            for asset in assets {
                let key = format!("{}{}", base_path, asset.realname);
                manifest.insert(key, asset.name.clone());
            }

            let manifest_json = serde_json::to_string_pretty(&manifest)?;

            let output_path = context.config.output.path.join(file_name);

            fs::write(output_path, manifest_json).unwrap();
        }
        Ok(None)
    }
}
