use glob::glob;
use serde_json::Value;
use std::io;
use std::path::PathBuf;

use crate::util::logger::log_verbose;

pub async fn find_workspaces(root_path: &PathBuf) -> io::Result<Vec<(String, PathBuf, Value)>> {
    let mut workspaces = Vec::new();

    // load package.json
    let pkg_path = root_path.join("package.json");
    let pkg_content = std::fs::read_to_string(pkg_path)?;
    let pkg: Value = serde_json::from_str(&pkg_content)?;

    // load workspaces config
    if let Some(workspaces_config) = pkg.get("workspaces") {
        match workspaces_config {
            Value::Array(patterns) => {
                for pattern in patterns {
                    if let Some(pattern_str) = pattern.as_str() {
                        // prepare glob pattern
                        let package_json_path = root_path.join(pattern_str).join("package.json");
                        let glob_pattern = package_json_path.to_str().ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid path encoding")
                        })?;

                        // glob match
                        for entry in glob(glob_pattern).map_err(|e| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("Invalid glob pattern: {}", e),
                            )
                        })? {
                            match entry {
                                Ok(path) => {
                                    // load package.json in workspace
                                    let workspace_content = std::fs::read_to_string(&path)?;
                                    let workspace_pkg: Value =
                                        serde_json::from_str(&workspace_content)?;

                                    // load workspace name
                                    let name =
                                        workspace_pkg["name"].as_str().unwrap_or("").to_string();

                                    // get workspace path
                                    let workspace_path = path
                                        .parent()
                                        .ok_or_else(|| {
                                            io::Error::new(
                                                io::ErrorKind::InvalidData,
                                                "Invalid workspace path",
                                            )
                                        })?
                                        .to_path_buf();

                                    log_verbose(&format!("Found workspace: {} {:?}", name, path));
                                    workspaces.push((name, workspace_path, workspace_pkg));
                                }
                                Err(e) => {
                                    log_verbose(&format!("Error processing workspace: {}", e))
                                }
                            }
                        }
                    }
                }
            }
            _ => log_verbose("Workspaces field is not an array"),
        }
    }

    Ok(workspaces)
}
