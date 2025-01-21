use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleFederationConfig {
    pub name: String,
    pub exposes: Option<ExposesConfig>,
    pub shared: Option<SharedConfig>,
    pub remotes: Option<RemotesConfig>,
    #[serde(default)]
    pub runtime_plugins: Vec<String>,
    pub implementation: String,
    #[serde(default)]
    pub share_strategy: ShareStrategy,
    #[serde(default = "default_share_scope")]
    pub share_scope: String,
}

pub type ExposesConfig = HashMap<String, String>;

pub type SharedConfig = HashMap<String, SharedItemConfig>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SharedItemConfig {
    #[serde(default)]
    /// not supported now
    pub eager: bool,
    #[serde(default)]
    pub singleton: bool,
    #[serde(default)]
    pub required_version: Option<String>,
    #[serde(default)]
    pub strict_version: bool,
    #[serde(default = "default_share_scope")]
    pub shared_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareStrategy {
    #[serde(rename = "version-first")]
    VersionFirst,
    #[serde(rename = "loaded-first")]
    LoadedFirst,
}

impl Default for ShareStrategy {
    fn default() -> Self {
        Self::LoadedFirst
    }
}

pub type RemotesConfig = HashMap<String, String>;

fn default_share_scope() -> String {
    "default".to_string()
}
