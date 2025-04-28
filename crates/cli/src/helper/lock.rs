use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, fs};
use std::path::PathBuf;

use crate::util::save_type::{PackageAction, SaveType};
use crate::{cmd::deps::build_deps, util::logger::log_info};
use crate::util::registry::resolve;

use super::workspace::find_workspaces;

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

pub async fn update_package_json(
    action: &PackageAction,
    spec: &str,
    workspace: &Option<String>,
    save_type: &SaveType,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse package spec
    let (name, version) = parse_package_spec(spec).await?;

    // 2. Find target workspace if specified
    let target_dir = if let Some(ws) = workspace {
        find_workspace_path(&PathBuf::from("."), &ws).await?
    } else {
        PathBuf::from(".")
    };

    // 3. Update package.json
    let package_json_path = target_dir.join("package.json");
    let mut package_json: Value = serde_json::from_reader(fs::File::open(&package_json_path)?)?;

    let dep_field = match save_type {
        SaveType::Dev => "devDependencies",
        SaveType::Peer => "peerDependencies",
        SaveType::Optional => "optionalDependencies",
        SaveType::Prod => "dependencies",
    };

    if let Some(deps) = package_json.get_mut(dep_field) {
        if let Some(deps_obj) = deps.as_object_mut() {
            match action {
                PackageAction::Add => {
                    deps_obj.insert(name.clone(), Value::String(version.clone()));
                }
                PackageAction::Remove => {
                    deps_obj.remove(&name);
                }
            }
        }
    } else if PackageAction::Add == *action {
        let mut deps = serde_json::Map::new();
        deps.insert(name.clone(), Value::String(version.clone()));
        package_json[dep_field] = Value::Object(deps);
    }

    // Write back to package.json
    fs::write(&package_json_path, serde_json::to_string_pretty(&package_json)?)?;

    // 4. Rebuild package-lock.json
    build_deps().await?;

    Ok(())
}

async fn parse_package_spec(spec: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = spec.split('@').collect();
    let name = parts[0].to_string();
    let version_spec = parts.get(1).map(|s| s.to_string()).unwrap_or("*".to_string());
    let resolved = resolve(&name, &version_spec).await?;
    Ok((name, resolved.version))
}

async fn find_workspace_path(cwd: &PathBuf, workspace: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let workspaces = find_workspaces(cwd).await?;
    for (name, path, _) in workspaces {
        if name == workspace || path.to_string_lossy() == workspace {
            return Ok(path);
        }
    }
    Err(format!("Workspace '{}' not found", workspace).into())
}
