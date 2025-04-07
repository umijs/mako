use std::sync::OnceLock;
use utoo_core::config::Config;

use super::logger::log_verbose;

static REGISTRY: OnceLock<String> = OnceLock::new();

pub fn set_registry(registry: Option<String>) {
    if let Some(registry) = registry {
        if !registry.trim().is_empty() {
            log_verbose(&format!("set registry: {}", registry));
            let _ = REGISTRY.set(registry);
        }
    }
}

pub fn get_registry() -> &'static str {
    if let Some(registry) = REGISTRY.get() {
        if !registry.is_empty() {
            return registry.as_str();
        }
    }

    // load from utoo-core config
    if let Ok(config) = Config::load(false) {
        if let Ok(Some(registry)) = config.get("registry") {
            let _ = REGISTRY.set(registry);
            return REGISTRY.get().unwrap().as_str();
        }
    }

    // default to npmmirror
    "https://registry.npmmirror.com"
}
