use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, fs};

use crate::{cmd::deps::build_deps, util::logger::log_info};

#[derive(Deserialize)]
pub struct PackageLock {
    pub packages: HashMap<String, Package>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Package {
    pub version: Option<String>,
    pub resolved: Option<String>,
    pub link: Option<bool>,
    pub cpu: Option<Value>,
    pub os: Option<Value>,
}

pub fn group_by_depth(
    packages: &HashMap<String, Package>,
) -> HashMap<usize, Vec<(String, Package)>> {
    let mut groups = HashMap::new();
    for (path, package) in packages {
        let depth = path.matches("node_modules").count();
        groups
            .entry(depth)
            .or_insert_with(Vec::new)
            .push((path.clone(), package.clone()));
    }
    groups
}

pub fn extract_package_name(path: &str) -> String {
    if let Some(index) = path.rfind("node_modules/") {
        let (_, package_path) = path.split_at(index + "node_modules/".len());
        package_path.to_string()
    } else {
        path.to_string()
    }
}

pub async fn ensure_package_lock() -> Result<(), String> {
    // check package.json exists in cwd
    if !fs::metadata("package.json").is_ok() {
        return Err("package.json not found".to_string());
    }
    // check package-lock.json exists in cwd
    if !fs::metadata("package-lock.json").is_ok() {
        log_info("Resolving dependencies");
        let _ = build_deps().await;
        Ok(())
    } else {
        // load package-lock.json directly if exists
        log_info("Loading package-lock.json from current project for dependency download");
        Ok(())
    }
}
