use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
pub enum DevtoolConfig {
    /// Generate separate sourcemap file
    #[serde(rename = "source-map")]
    SourceMap,
    /// Generate inline sourcemap
    #[serde(rename = "inline-source-map")]
    InlineSourceMap,
}

create_deserialize_fn!(deserialize_devtool, DevtoolConfig);
