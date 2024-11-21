#[derive(Clone, Serialize, Deserialize)]
pub struct SharedItem {
    name: String,
    version: Option<SharedVersion>,
    required_version: Option<SharedVersion>,
    strict_version: Option<bool>,
    singleton: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SharedVersion {
    Version(String),
    False,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SharedOption {
    shared: HashMap<String, SharedItem>,
    shared_scope: String,
}
