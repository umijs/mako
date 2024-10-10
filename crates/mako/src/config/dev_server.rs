use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DevServerConfig {
    pub host: String,
    pub port: u16,
}

create_deserialize_fn!(deserialize_dev_server, DevServerConfig);
