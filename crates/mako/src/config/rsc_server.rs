use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RscServerConfig {
    pub client_component_tpl: String,
    #[serde(rename = "emitCSS")]
    pub emit_css: bool,
}

create_deserialize_fn!(deserialize_rsc_server, RscServerConfig);
