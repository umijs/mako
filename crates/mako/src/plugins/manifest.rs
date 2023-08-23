use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use anyhow::Result;
use regex::Regex;
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
            let file_name = context.config.manifest_config.file_name.clone();
            let base_path = context.config.manifest_config.base_path.clone();

            let path = normalize_path(base_path);

            for asset in assets {
                let key = format!("{}{}", path, remove_key_hash(&asset.hashname));
                manifest.insert(key, asset.hashname.clone());
            }

            let manifest_json = serde_json::to_string_pretty(&manifest)?;

            let output_path = context.config.output.path.join(file_name);

            fs::write(output_path, manifest_json).unwrap();
        }
        Ok(None)
    }
}

fn normalize_path(mut path: String) -> String {
    if !path.is_empty() && !path.ends_with('/') {
        path.push('/');
    }

    path
}

fn remove_key_hash(key: &str) -> String {
    // 需要确定是使用 md5 算法产生的 hash 才能保证是 32 长度
    let reg = Regex::new(r"[a-fA-F0-9]{32}\.?").unwrap();
    let val = reg.replace_all(key, "").to_string();
    val
}
