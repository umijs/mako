use std::sync::OnceLock;

static REGISTRY: OnceLock<String> = OnceLock::new();

pub fn set_registry(registry: &str) {
    let _ = REGISTRY.set(registry.to_string());
}

pub fn get_registry() -> &'static str {
    REGISTRY
        .get()
        .map(|s| s.as_str())
        .unwrap_or("https://registry.npmmirror.com")
}
