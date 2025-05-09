use crate::helper::{package::serialize_tree_to_packages, ruborist::Ruborist};
use crate::util::config::get_legacy_peer_deps;
use crate::util::logger::log_warning;
use crate::util::semver;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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

pub async fn build_deps() -> Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_ideal_tree().await?;

    // let _ = ruborist.print_tree();

    if let Some(ideal_tree) = &ruborist.ideal_tree {
        // Add reference
        // Create package-lock.json structure
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

        fs::rename(temp_path, target_path)
            .context("Failed to rename temporary package-lock.json")?;
    }

    validate_deps()?;
    Ok(())
}

pub async fn build_workspace() -> Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_workspace_tree().await?;

    if let Some(ideal_tree) = &ruborist.ideal_tree {
        let mut node_list = Vec::new();
        let mut edges = Vec::new();
        let mut workspace_names = HashSet::new();

        for child in ideal_tree.children.read().unwrap().iter() {
            let name = child.name.clone();
            if child.is_link {
                continue;
            }
            workspace_names.insert(name.clone());
            node_list.push(json!({
                "name": name,
                "path": child.path.clone(),
            }));
        }

        for child in ideal_tree.children.read().unwrap().iter() {
            for edge in child.edges_out.read().unwrap().iter() {
                if *edge.valid.read().unwrap() {
                    if let Some(to_node) = edge.to.read().unwrap().as_ref() {
                        edges.push(json!([to_node.name.clone(), edge.from.name.clone()]));
                    }
                }
            }
        }

        let workspace_file = json!({
            "nodeList": node_list,
            "edges": edges,
        });

        let temp_path = path.join("workspace.json.tmp");
        let target_path = path.join("workspace.json");

        fs::write(&temp_path, serde_json::to_string_pretty(&workspace_file)?)
            .context("Failed to write temporary workspace.json")?;
        fs::rename(temp_path, target_path).context("Failed to rename temporary workspace.json")?;
    }

    Ok(())
}

fn validate_deps() -> Result<()> {
    let path = PathBuf::from(".");
    let lock_path = path.join("package-lock.json");

    let lock_content = fs::read_to_string(lock_path).context("Failed to read package-lock.json")?;
    let lock_file: serde_json::Value =
        serde_json::from_str(&lock_content).context("Failed to parse package-lock.json")?;

    if let Some(packages) = lock_file.get("packages").and_then(|p| p.as_object()) {
        for (pkg_path, pkg_info) in packages {
            for (dep_field, is_optional) in get_dep_types() {
                if let Some(dependencies) = pkg_info.get(dep_field).and_then(|d| d.as_object()) {
                    for (dep_name, req_version) in dependencies {
                        let req_version_str = req_version.as_str().unwrap_or_default();

                        // find the actual version of the dependency
                        let mut current_path = if pkg_path.is_empty() {
                            String::new()
                        } else {
                            pkg_path.to_string()
                        };
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
                                if !semver::matches(&req_version_str, &actual_version) {
                                    log_warning(&format!(
                                        "Package {} {} dependency {} (required version: {}) does not match actual version {}@{}",
                                        pkg_path, dep_field, dep_name, req_version_str, current_path, actual_version
                                    ));
                                }
                            }
                        } else if !is_optional {
                            log_warning(&format!(
                                "pkg_path {} dep_field {} dep_name {} not found",
                                pkg_path, dep_field, dep_name
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
