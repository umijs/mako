use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum TreeShakingStrategy {
    #[serde(rename = "basic")]
    Basic,
    #[serde(rename = "advanced")]
    Advanced,
}

create_deserialize_fn!(deserialize_tree_shaking, TreeShakingStrategy);
