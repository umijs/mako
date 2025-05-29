use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, fs};

use crate::util::config::get_legacy_peer_deps;
use crate::util::json::{load_package_json, load_package_lock_json};
use crate::util::logger::{log_verbose, log_warning};
use crate::util::node::{Node, Overrides};
use crate::util::registry::resolve;
use crate::util::save_type::{PackageAction, SaveType};
use crate::util::semver;
use crate::util::{cache::parse_pattern, cloner::clone, downloader::download};
use crate::{cmd::deps::build_deps, util::logger::log_info};

use super::workspace::find_workspace_path;

#[derive(Deserialize)]
pub struct PackageLock {
    pub packages: HashMap<String, Package>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Package {
    pub version: Option<String>,
    pub resolved: Option<String>,
    pub link: Option<bool>,
    pub cpu: Option<Value>,
    pub os: Option<Value>,
    pub has_install_script: Option<bool>,
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
    if fs::metadata("package.json").is_err() {
        return Err(anyhow!("package.json not found"));
    }
    // check package-lock.json exists in cwd
    if fs::metadata("package-lock.json").is_err() {
        log_info("Resolving dependencies");
        build_deps().await?;
        Ok(())
    } else {
        // load package-lock.json directly if exists
        log_info("Loading package-lock.json from current project for dependency download");
        // Validate dependencies to ensure package-lock.json is in sync with package.json
        if let Err(e) = validate_deps().await {
            log_info(&format!("package-lock.json is outdated, {}", e));
            build_deps().await?;
        }
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
        find_workspace_path(&PathBuf::from("."), ws)
            .await
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
    build_deps()
        .await
        .map_err(|e| anyhow!("Failed to rebuild dependencies: {}", e))?;

    Ok(())
}

pub async fn parse_package_spec(spec: &str) -> Result<(String, String, String)> {
    let (name, version_spec) = parse_pattern(spec);
    let resolved = resolve(&name, &version_spec).await?;
    Ok((name, resolved.version, version_spec))
}

pub async fn prepare_global_package_json(npm_spec: &str) -> Result<PathBuf> {
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
        download(tarball_url, &cache_path)
            .await
            .map_err(|e| anyhow!("Failed to download package: {}", e))?;

        // If the package has install scripts, create a flag file
        // in linux, we can use hardlink when FICLONE is not supported
        // so we need to copy the file to the package directory to avoid effect other packages
        if resolved.manifest.get("hasInstallScript") == Some(&json!(true)) {
            let has_install_script_flag_path = cache_path.join("_hasInstallScript");
            fs::write(has_install_script_flag_path, "")?;
        }
    }

    // Clone to package directory
    log_verbose(&format!(
        "Cloning {} to {}",
        cache_path.display(),
        package_path.display()
    ));
    clone(&cache_path, &package_path, true)
        .await
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

/// Extract the relative package name from a package directory path string.
/// Handles both normal and scoped packages, and skips invalid deep paths.
pub fn path_to_pkg_name(path_str: &str) -> Option<&str> {
    if let Some(idx) = path_str.rfind("node_modules/") {
        let pkg_name = &path_str[idx + "node_modules/".len()..];
        let parts: Vec<&str> = pkg_name.split('/').collect();
        // Only allow ora or @scope/ora, skip @pkg/name/path/custom/package.json
        if parts.len() > 2 || (parts.len() == 2 && !parts[0].starts_with('@')) {
            return None;
        }
        Some(pkg_name)
    } else {
        None
    }
}

#[derive(Debug)]
pub struct InvalidDependency {
    pub package_path: String,
    pub dependency_name: String,
}

pub async fn validate_deps() -> Result<Vec<InvalidDependency>> {
    let mut invalid_deps = Vec::new();
    // Read package.json for overrides
    let pkg_file = load_package_json()?;
    // Initialize overrides
    let overrides = Overrides::new(pkg_file.clone()).parse(pkg_file.clone());

    let lock_file = load_package_lock_json()?;

    // check package-lock.json packages and package.json dependencies are the same
    let pkg_in_pkg_lock = lock_file
        .get("packages")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get(""));

    if let Some(root_pkg) = pkg_in_pkg_lock {
        for (dep_field, _is_optional) in get_dep_types() {
            if root_pkg.get(dep_field) != pkg_file.get(dep_field) {
                return Ok(invalid_deps);
            }
        }
    }

    if let Some(packages) = lock_file.get("packages").and_then(|p| p.as_object()) {
        for (pkg_path, pkg_info) in packages {
            for (dep_field, is_optional) in get_dep_types() {
                if let Some(dependencies) = pkg_info.get(dep_field).and_then(|d| d.as_object()) {
                    for (dep_name, req_version) in dependencies {
                        let req_version_str = req_version.as_str().unwrap_or_default();

                        // Collect parent chain information
                        let mut parent_chain = Vec::new();
                        let mut current_path = String::from(pkg_path);

                        while !current_path.is_empty() {
                            if let Some(pkg_info) = packages.get(&current_path) {
                                if let Some(name) = pkg_info.get("name").and_then(|n| n.as_str()) {
                                    if let Some(version) =
                                        pkg_info.get("version").and_then(|v| v.as_str())
                                    {
                                        parent_chain.push((name.to_string(), version.to_string()));
                                    }
                                }
                            }

                            if let Some(last_modules) = current_path.rfind("/node_modules/") {
                                current_path = current_path[..last_modules].to_string();
                            } else {
                                current_path = String::new();
                            }
                        }

                        // Check if there's an override rule for this dependency
                        let effective_req_version = if let Some(overrides) = &overrides {
                            let mut effective_version = req_version_str.to_string();
                            for rule in &overrides.rules {
                                if overrides
                                    .matches_rule(rule, dep_name, req_version_str, &parent_chain)
                                    .await
                                {
                                    effective_version = rule.target_spec.clone();
                                    break;
                                }
                            }
                            effective_version
                        } else {
                            req_version_str.to_string()
                        };

                        // find the actual version of the dependency
                        let mut current_path = String::from(pkg_path);
                        let mut dep_info = None;

                        // until root or found
                        loop {
                            let search_path = if current_path.is_empty() {
                                format!("node_modules/{}", dep_name)
                            } else {
                                format!("{}/node_modules/{}", current_path, dep_name)
                            };

                            if let Some(info) = packages.get(&search_path) {
                                dep_info = Some(info);
                                current_path = search_path;
                                break;
                            }

                            // find in root path
                            if current_path.is_empty() {
                                break;
                            }

                            // find in parent path
                            if let Some(last_modules) = current_path.rfind("/node_modules/") {
                                current_path = current_path[..last_modules].to_string();
                            } else {
                                current_path = String::new();
                            }
                        }

                        // optional dependency not found is allowed
                        if let Some(dep_info) = dep_info {
                            if let Some(actual_version) =
                                dep_info.get("version").and_then(|v| v.as_str())
                            {
                                if !semver::matches(&effective_req_version, actual_version) {
                                    log_warning(&format!(
                                        "Package {} {} dependency {} (required version: {}, effective version: {}) does not match actual version {}@{}",
                                        pkg_path, dep_field, dep_name, req_version_str, effective_req_version, current_path, actual_version
                                    ));
                                    invalid_deps.push(InvalidDependency {
                                        package_path: pkg_path.clone(),
                                        dependency_name: dep_name.clone(),
                                    });
                                }
                            }
                        } else if !is_optional {
                            log_warning(&format!(
                                "pkg_path {} dep_field {} dep_name {} not found",
                                pkg_path, dep_field, dep_name
                            ));
                            invalid_deps.push(InvalidDependency {
                                package_path: pkg_path.clone(),
                                dependency_name: dep_name.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(invalid_deps)
}

fn get_dep_types() -> Vec<(&'static str, bool)> {
    let legacy_peer_deps = get_legacy_peer_deps();

    if legacy_peer_deps {
        vec![
            ("dependencies", false),
            ("optionalDependencies", true),
            ("devDependencies", false),
        ]
    } else {
        vec![
            ("dependencies", false),
            ("peerDependencies", false),
            ("optionalDependencies", true),
            ("devDependencies", false),
        ]
    }
}

pub async fn write_ideal_tree_to_lock_file(ideal_tree: &Arc<Node>) -> Result<()> {
    let path = PathBuf::from(".");
    let lock_file = json!({
        "name": ideal_tree.name,  // Direct field access
        "version": ideal_tree.version,  // Direct field access
        "lockfileVersion": 3,
        "requires": true,
        "packages": serialize_tree_to_packages(ideal_tree),
    });

    // Write to temporary file first, then atomically move to target location
    let temp_path = path.join("package-lock.json.tmp");
    let target_path = path.join("package-lock.json");

    fs::write(&temp_path, serde_json::to_string_pretty(&lock_file)?)
        .context("Failed to write temporary package-lock.json")?;

    fs::rename(temp_path, target_path).context("Failed to rename temporary package-lock.json")?;

    Ok(())
}

pub fn serialize_tree_to_packages(node: &Arc<Node>) -> Value {
    let mut packages = json!({});
    let mut stack = vec![(node.clone(), String::new())];
    let mut total_packages = 0;

    while let Some((current, prefix)) = stack.pop() {
        let children = current.children.read().unwrap();
        let mut name_count = HashMap::new();
        for child in children.iter() {
            if !child.is_link {
                *name_count.entry(child.name.as_str()).or_insert(0) += 1;
            }
        }
        for (name, count) in name_count {
            if count > 1 {
                log_warning(&format!(
                    "Found {} duplicate dependencies named '{}' under '{}'",
                    count, name, current.name
                ));
            }
        }
        let mut pkg_info = if current.is_root {
            json!({
                "name": current.name,
                "version": current.version,
            })
        } else {
            let mut info = json!({
                "name": current.package.get("name"),
            });

            if current.is_workspace {
            } else if current.is_link {
                // update resolved field
                info["link"] = json!(true);
                // resolvd => targetNode#path
                info["resolved"] = json!(current
                    .target
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .path
                    .to_string_lossy());
            } else {
                info["version"] = json!(current.package.get("version"));
                info["resolved"] = json!(current
                    .package
                    .get("dist")
                    .unwrap_or(&json!(""))
                    .get("tarball"));
                info["integrity"] = json!(current
                    .package
                    .get("dist")
                    .unwrap_or(&json!(""))
                    .get("integrity"));
                total_packages += 1;
            }

            if *current.is_peer.read().unwrap() == Some(true) {
                info["peer"] = json!(true);
            }

            let is_dev = *current.is_dev.read().unwrap() == Some(true);
            let is_optional = *current.is_optional.read().unwrap() == Some(true);

            if is_dev && is_optional {
                info["devOptional"] = json!(true);
            } else if is_dev {
                info["dev"] = json!(true);
            } else if is_optional {
                info["optional"] = json!(true);
            }

            // hasBin
            if current.package.get("hasInstallScript") == Some(&json!(true)) {
                info["hasInstallScript"] = json!(true);
            }

            info
        };

        // add dependencies field
        let fields = if current.is_link {
            vec![]
        } else if current.is_root || current.is_workspace {
            vec![
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ]
        } else {
            vec![
                "dependencies",
                "peerDependencies",
                "optionalDependencies",
                "bin",
                "license",
                "engines",
                "os",
                "cpu",
            ]
        };

        for field in fields.iter() {
            if let Some(deps) = current.package.get(field) {
                if deps.is_object() {
                    if !deps.as_object().unwrap().is_empty() {
                        pkg_info[field] = deps.clone();
                    }
                } else {
                    // compatible for string type
                    pkg_info[field] = deps.clone();
                }
            }
        }

        // use "" for root node
        let key = if prefix.is_empty() {
            "".to_string()
        } else {
            prefix.clone()
        };
        packages[key] = pkg_info;

        // process children
        let children = current.children.read().unwrap();

        for child in children.iter() {
            let child_prefix = if prefix.is_empty() {
                if child.is_workspace {
                    format!("{}", child.path.to_string_lossy())
                } else {
                    format!("node_modules/{}", child.name)
                }
            } else {
                format!("{}/node_modules/{}", &prefix, child.name)
            };
            stack.push((child.clone(), child_prefix));
        }
    }

    log_info(&format!(
        "Total {} dependencies after merging",
        total_packages
    ));
    packages
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn test_path_to_pkg_name() {
        // Normal nested package
        assert_eq!(
            super::path_to_pkg_name("/root/node_modules/a/node_modules/b"),
            Some("b")
        );
        // Top-level package
        assert_eq!(super::path_to_pkg_name("/root/node_modules/a"), Some("a"));

        assert_eq!(
            super::path_to_pkg_name("/root/node_modules/@a/b"),
            Some("@a/b")
        );
        // Deep invalid path (should be None)
        assert_eq!(
            super::path_to_pkg_name("/root/node_modules/@a/b/node_modules/b/c/d"),
            None
        );
    }
}
