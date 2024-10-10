use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalConfig {
    pub webpack_syntax_validate: Vec<String>,
    pub require_context: bool,
    #[serde(deserialize_with = "deserialize_detect_loop")]
    pub detect_circular_dependence: Option<DetectCircularDependence>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetectCircularDependence {
    pub ignores: Vec<String>,
    pub graphviz: bool,
}

create_deserialize_fn!(deserialize_detect_loop, DetectCircularDependence);