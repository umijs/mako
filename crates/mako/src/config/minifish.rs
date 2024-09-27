use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinifishConfig {
    pub mapping: HashMap<String, String>,
    pub meta_path: Option<PathBuf>,
    pub inject: Option<HashMap<String, InjectItem>>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InjectItem {
    pub from: String,
    pub named: Option<String>,
    pub namespace: Option<bool>,
    pub exclude: Option<String>,
    pub include: Option<String>,
    pub prefer_require: Option<bool>,
}

create_deserialize_fn!(deserialize_minifish, MinifishConfig);
