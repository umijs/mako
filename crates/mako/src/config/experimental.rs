use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RustPlugin {
    pub path: String,
    pub options: Value,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalConfig {
    pub webpack_syntax_validate: Vec<String>,
    pub require_context: bool,
    // this feature is conflicting with require_context
    pub ignore_non_literal_require: bool,
    pub magic_comment: bool,
    #[serde(deserialize_with = "deserialize_detect_loop")]
    pub detect_circular_dependence: Option<DetectCircularDependence>,
    pub rust_plugins: Vec<RustPlugin>,
    pub central_ensure: bool,
    pub imports_checker: bool,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetectCircularDependence {
    pub ignores: Vec<String>,
    pub graphviz: bool,
}

create_deserialize_fn!(deserialize_detect_loop, DetectCircularDependence);
