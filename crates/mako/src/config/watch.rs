use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatchConfig {
    pub ignore_paths: Option<Vec<String>>,
    #[serde(rename = "_nodeModulesRegexes")]
    pub node_modules_regexes: Option<Vec<String>>,
}
