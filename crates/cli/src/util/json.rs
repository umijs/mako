use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use std::fs::File;
use std::path::Path;
use serde_json::Value;

/// Read and parse a JSON file into the specified type
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path)
        .map_err(|e| anyhow!("Failed to open file {}: {}", path.display(), e))?;

    serde_json::from_reader(file)
        .map_err(|e| anyhow!("Failed to parse JSON from {}: {}", path.display(), e))
}

/// Read and parse a JSON file into a serde_json::Value
pub fn read_json_value(path: &Path) -> Result<serde_json::Value> {
    read_json_file(path)
}

/// Load package.json from current directory
pub fn load_package_json() -> Result<Value> {
    read_json_value(Path::new("package.json"))
}

pub fn load_package_lock_json() -> Result<Value> {
    read_json_value(Path::new("package-lock.json"))
}

/// Load package.json from specified path
pub fn load_package_json_from_path(path: &Path) -> Result<Value> {
    read_json_value(&path.join("package.json"))
}
