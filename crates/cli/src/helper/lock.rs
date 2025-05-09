use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::{collections::HashMap, fs};
use anyhow::{Result, anyhow};

use crate::util::logger::log_verbose;
use crate::util::registry::resolve;
use crate::util::save_type::{PackageAction, SaveType};
use crate::util::{cache::parse_pattern, cloner::clone, downloader::download};
use crate::{cmd::deps::build_deps, util::logger::log_info};

use super::workspace::find_workspace_path;

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

pub async fn ensure_package_lock() -> Result<()> {
    // check package.json exists in cwd
    if !fs::metadata("package.json").is_ok() {
        return Err(anyhow!("package.json not found"));
    }
    // check package-lock.json exists in cwd
    if !fs::metadata("package-lock.json").is_ok() {
        log_info("Resolving dependencies");
        build_deps().await.map_err(|e| anyhow!("Failed to build dependencies: {}", e))?;
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
) -> Result<()> {
    // 1. Parse package spec
    let (name, version, version_spec) = parse_package_spec(spec).await?;

    // 2. Find target workspace if specified
    let target_dir = if let Some(ws) = workspace {
        find_workspace_path(&PathBuf::from("."), &ws).await
            .map_err(|e| anyhow!("Failed to find workspace path: {}", e))?
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

    let version_to_write = match version_spec {
        spec if spec.is_empty() || spec == "*" || spec == "latest" => format!("^{}", version),
        spec => spec.to_string(),
    };

    if let Some(deps) = package_json.get_mut(dep_field) {
        if let Some(deps_obj) = deps.as_object_mut() {
            match action {
                PackageAction::Add => {
                    deps_obj.insert(name.clone(), Value::String(version_to_write.clone()));
                }
                PackageAction::Remove => {
                    deps_obj.remove(&name);
                }
            }
        }
    } else if PackageAction::Add == *action {
        let mut deps = serde_json::Map::new();
        deps.insert(name.clone(), Value::String(version_to_write));
        package_json[dep_field] = Value::Object(deps);
    }

    // Write back to package.json
    fs::write(
        &package_json_path,
        serde_json::to_string_pretty(&package_json)?,
    )?;

    // 4. Rebuild package-lock.json
    build_deps().await.map_err(|e| anyhow!("Failed to rebuild dependencies: {}", e))?;

    Ok(())
}

pub async fn parse_package_spec(
    spec: &str,
) -> Result<(String, String, String)> {
    let (name, version_spec) = parse_pattern(spec);
    let resolved = resolve(&name, &version_spec).await?;
    Ok((name, resolved.version, version_spec))
}

pub async fn prepare_global_package_json(
    npm_spec: &str,
) -> Result<PathBuf> {
    // Parse package name and version
    let (name, _version, version_spec) = parse_package_spec(npm_spec).await?;

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
    let resolved = resolve(&name, &version_spec).await?;

    // Get tarball URL from manifest
    let tarball_url = resolved.manifest["dist"]["tarball"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get tarball URL from manifest"))?;

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
        download(tarball_url, &cache_path).await
            .map_err(|e| anyhow!("Failed to download package: {}", e))?;
    }

    // Clone to package directory
    log_verbose(&format!(
        "Cloning {} to {}",
        cache_path.display(),
        package_path.display()
    ));
    clone(&cache_path, &package_path, true).await
        .map_err(|e| anyhow!("Failed to clone package: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_to_write() {
        // Test cases for different version specifications
        let test_cases = vec![
            ("1.2.3", "", "^1.2.3"),
            ("1.2.3", "*", "^1.2.3"),
            ("1.2.3", "latest", "^1.2.3"),
            ("1.2.3", "^1.2.0", "^1.2.0"),
            ("1.2.3", "~1.2.0", "~1.2.0"),
            ("1.2.3", "1.2.3", "1.2.3"),
        ];

        for (version, spec, expected) in test_cases {
            let version_to_write = match spec {
                spec if spec.is_empty() || spec == "*" || spec == "latest" => {
                    format!("^{}", version)
                }
                spec => spec.to_string(),
            };
            assert_eq!(
                version_to_write, expected,
                "Failed for version: {}, spec: {}",
                version, spec
            );
        }
    }
}
