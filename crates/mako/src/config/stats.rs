use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct StatsConfig {
    pub modules: bool,
}

create_deserialize_fn!(deserialize_stats, StatsConfig);
