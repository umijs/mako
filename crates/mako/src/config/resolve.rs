use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct ResolveConfig {
    pub alias: Vec<(String, String)>,
    pub extensions: Vec<String>,
}
