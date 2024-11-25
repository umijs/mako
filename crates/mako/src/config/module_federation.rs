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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SharedConfig {
    singleton: Option<bool>,
    required_version: Option<SharedVersion>,
    shared_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SharedVersion {
    Version(String),
    False,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareStrategy {
    #[serde(rename = "version_first")]
    VersionFirst,
    #[serde(rename = "loaded_first")]
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
