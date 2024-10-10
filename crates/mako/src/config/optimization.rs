use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationConfig {
    pub skip_modules: Option<bool>,
    pub concatenate_modules: Option<bool>,
}

create_deserialize_fn!(deserialize_optimization, OptimizationConfig);
