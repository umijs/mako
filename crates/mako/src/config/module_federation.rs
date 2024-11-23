use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleFederationConfig {
    pub exposes: Option<ExposesConfig>,
    pub shared: Option<SharedConfig>,
    pub remotes: Option<RemotesConfig>,
    #[serde(default)]
    pub runtime_plugins: Vec<String>,
    pub implementation: String,
}

pub type ExposesConfig = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub type RemotesConfig = HashMap<String, String>;
