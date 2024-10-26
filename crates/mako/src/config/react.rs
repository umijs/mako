use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct ReactConfig {
    pub pragma: String,
    #[serde(rename = "importSource")]
    pub import_source: String,
    pub runtime: ReactRuntimeConfig,
    #[serde(rename = "pragmaFrag")]
    pub pragma_frag: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum ReactRuntimeConfig {
    #[serde(rename = "automatic")]
    Automatic,
    #[serde(rename = "classic")]
    Classic,
}
