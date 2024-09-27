use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ProgressConfig {
    #[serde(rename = "progressChars", default)]
    pub progress_chars: String,
}

create_deserialize_fn!(deserialize_progress, ProgressConfig);
