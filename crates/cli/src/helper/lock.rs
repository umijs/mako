use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::{collections::HashMap, fs};

use crate::util::logger::log_verbose;
use crate::util::registry::resolve;
use crate::util::save_type::{PackageAction, SaveType};
use crate::util::{cache::parse_pattern, cloner::clone, downloader::download};
use crate::{cmd::deps::build_deps, util::logger::log_info};

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
    fs::write(
        &package_json_path,
        serde_json::to_string_pretty(&package_json)?,
    )?;

    // 4. Rebuild package-lock.json
    build_deps().await?;

    Ok(())
}

pub async fn parse_package_spec(
    spec: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let (name, version_spec) = parse_pattern(spec);
    let resolved = resolve(&name, &version_spec).await?;
    Ok((name, resolved.version))
}

async fn find_workspace_path(
    cwd: &PathBuf,
    workspace: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let workspaces = find_workspaces(cwd).await?;
    for (name, path, _) in workspaces {
        if name == workspace || path.to_string_lossy() == workspace {
            return Ok(path);
        }
    }
    Err(format!("Workspace '{}' not found", workspace).into())
}

pub async fn prepare_global_package_json(
    npm_spec: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Parse package name and version
    let (name, spec) = parse_package_spec(npm_spec).await?;

    // Get current executable path
    let current_exe = std::env::current_exe()?;
    let lib_path = current_exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/node_modules");
    log_verbose(&format!("lib_path: {}", lib_path.to_string_lossy()));

    // Create global package directory
    let package_path = lib_path.join(&name);
    tokio::fs::create_dir_all(&package_path).await?;

    // Get package info from registry
    let resolved = resolve(&name, &spec).await?;

    // Get tarball URL from manifest
    let tarball_url = resolved.manifest["dist"]["tarball"]
        .as_str()
        .ok_or_else(|| "Failed to get tarball URL from manifest")?;

    // Download and extract package
    let cache_dir = crate::util::cache::get_cache_dir();
    let cache_path = cache_dir.join(format!("{}/{}", name, resolved.version));
    let cache_flag_path = cache_dir.join(format!("{}/{}/_resolved", name, resolved.version));

    // Download if not cached
    if !cache_flag_path.exists() {
        log_verbose(&format!(
            "Downloading {} to {}",
            tarball_url,
            cache_path.display()
        ));
        download(tarball_url, &cache_path).await?;
    }

    // Clone to package directory
    log_verbose(&format!(
        "Cloning {} to {}",
        cache_path.display(),
        package_path.display()
    ));
    clone(&cache_path, &package_path, true).await?;

    // Remove devDependencies, peerDependencies and optionalDependencies from package.json
    let package_json_path = package_path.join("package.json");
    let mut package_json: Value = serde_json::from_reader(fs::File::open(&package_json_path)?)?;

    // Remove specified dependency fields
    package_json
        .as_object_mut()
        .unwrap()
        .remove("devDependencies");

    // Write back the modified package.json
    fs::write(
        &package_json_path,
        serde_json::to_string_pretty(&package_json)?,
    )?;

    log_verbose(&format!("package_path: {}", package_path.to_string_lossy()));
    Ok(package_path)
}
