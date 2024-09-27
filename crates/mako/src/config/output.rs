use core::fmt;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use swc_core::ecma::ast::EsVersion;

use crate::create_deserialize_fn;
use crate::utils::get_pkg_name;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub path: PathBuf,
    pub mode: OutputMode,
    pub es_version: EsVersion,
    pub meta: bool,
    pub chunk_loading_global: String,
    pub preserve_modules: bool,
    pub preserve_modules_root: PathBuf,
    pub skip_write: bool,
    #[serde(deserialize_with = "deserialize_cross_origin_loading")]
    pub cross_origin_loading: Option<CrossOriginLoading>,
    pub global_module_registry: bool,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum OutputMode {
    #[serde(rename = "bundle")]
    Bundle,
    #[serde(rename = "bundless")]
    Bundless,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum CrossOriginLoading {
    #[serde(rename = "anonymous")]
    Anonymous,
    #[serde(rename = "use-credentials")]
    UseCredentials,
}

impl fmt::Display for CrossOriginLoading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrossOriginLoading::Anonymous => write!(f, "anonymous"),
            CrossOriginLoading::UseCredentials => write!(f, "use-credentials"),
        }
    }
}

pub fn get_default_chunk_loading_global(umd: Option<String>, root: &Path) -> String {
    let unique_name = umd.unwrap_or_else(|| get_pkg_name(root).unwrap_or("global".to_string()));

    format!("makoChunk_{}", unique_name)
}

create_deserialize_fn!(deserialize_cross_origin_loading, CrossOriginLoading);
