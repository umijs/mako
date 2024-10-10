use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DuplicatePackageCheckerConfig {
    #[serde(rename = "verbose", default)]
    pub verbose: bool,
    #[serde(rename = "emitError", default)]
    pub emit_error: bool,
    #[serde(rename = "showHelp", default)]
    pub show_help: bool,
}

create_deserialize_fn!(
    deserialize_check_duplicate_package,
    DuplicatePackageCheckerConfig
);
