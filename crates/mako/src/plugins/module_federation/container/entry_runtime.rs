use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ExposesOption {
    name: String,
    import: String,
    shared_scope: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ContainerOptions {
    name: String,
    exposes: ExposesOption,
    runtime_plugins: Vec<String>,
    shared_scope: String,
}
