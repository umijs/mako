use serde::{Deserialize, Serialize};

use crate::{create_deserialize_fn, plugins};

#[derive(Deserialize, Serialize, Debug)]
pub struct ManifestConfig {
    #[serde(
        rename(deserialize = "fileName"),
        default = "plugins::manifest::default_manifest_file_name"
    )]
    pub file_name: String,
    #[serde(rename(deserialize = "basePath"), default)]
    pub base_path: String,
}

create_deserialize_fn!(deserialize_manifest, ManifestConfig);
