use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HmrConfig {}

create_deserialize_fn!(deserialize_hmr, HmrConfig);
