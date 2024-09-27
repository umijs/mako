use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RscClientConfig {
    pub log_server_component: LogServerComponent,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum LogServerComponent {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "ignore")]
    Ignore,
}

create_deserialize_fn!(deserialize_rsc_client, RscClientConfig);
