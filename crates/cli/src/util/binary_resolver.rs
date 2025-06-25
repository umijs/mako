use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;
use serde_json::Value;
use crate::util::package_installer::get_utoo_cache_dir;

/// Convert package name to safe directory name (same as in package_installer.rs)
fn package_name_to_dir_name(package_name: &str) -> String {
    package_name.replace("/", "_")
}

/// Find a binary in node_modules/.bin directories, searching up the directory tree
pub async fn find_binary(command: &str) -> Result<Option<PathBuf>> {
    let current_dir = std::env::current_dir()?;
    find_binary_in_hierarchy(&current_dir, command).await
}

/// Find a binary in the utoo cache directory
pub async fn find_binary_in_cache(command: &str, package_name: &str) -> Result<Option<PathBuf>> {
    let cache_dir = get_utoo_cache_dir()?;
    let package_cache_dir = cache_dir.join(package_name_to_dir_name(package_name));

    if !package_cache_dir.exists() {
        return Ok(None);
    }

    // First try to find the binary in node_modules/.bin (for dependencies and linked binaries)
    let node_modules_bin = package_cache_dir.join("node_modules").join(".bin");
    if node_modules_bin.exists() {
        let bin_path = node_modules_bin.join(command);
        if bin_path.exists() && is_executable(&bin_path).await? {
            return Ok(Some(bin_path));
        }

        // Try with .cmd extension on Windows
        if cfg!(windows) {
            let cmd_path = node_modules_bin.join(format!("{}.cmd", command));
            if cmd_path.exists() && is_executable(&cmd_path).await? {
                return Ok(Some(cmd_path));
            }
        }
    }

    // If not found in node_modules/.bin, check if the package itself provides the binary
    let package_json_path = package_cache_dir.join("package.json");
    if package_json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json_path) {
            if let Ok(package_json) = serde_json::from_str::<Value>(&content) {
                if let Some(bin_files) = get_bin_files_from_package_json(&package_json, package_name) {
                    for (bin_name, bin_path) in bin_files {
                        if bin_name == command {
                            let full_bin_path = package_cache_dir.join(&bin_path);
                            if full_bin_path.exists() && is_executable(&full_bin_path).await? {
                                return Ok(Some(full_bin_path));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Extract bin files from package.json
fn get_bin_files_from_package_json(package_json: &Value, package_name: &str) -> Option<Vec<(String, String)>> {
    match package_json.get("bin") {
        Some(Value::Object(obj)) => {
            let mut bin_files = Vec::new();
            for (k, v) in obj.iter() {
                if let Some(path) = v.as_str() {
                    bin_files.push((k.clone(), path.to_string()));
                }
            }
            Some(bin_files)
        }
        Some(Value::String(s)) => {
            // Extract package name from full package name
            let simple_name = if package_name.contains('/') {
                package_name.split('/').last().unwrap_or(package_name)
            } else {
                package_name
            };
            Some(vec![(simple_name.to_string(), s.clone())])
        }
        _ => None,
    }
}

/// Search for binary in the directory hierarchy starting from the given path
async fn find_binary_in_hierarchy(start_path: &Path, command: &str) -> Result<Option<PathBuf>> {
    let mut current_path = start_path.to_path_buf();

    loop {
        let bin_path = current_path.join("node_modules").join(".bin").join(command);

        // Check if the binary exists and is executable
        if bin_path.exists() && is_executable(&bin_path).await? {
            return Ok(Some(bin_path));
        }

        // Try with .cmd extension on Windows
        if cfg!(windows) {
            let cmd_path = current_path.join("node_modules").join(".bin").join(format!("{}.cmd", command));
            if cmd_path.exists() && is_executable(&cmd_path).await? {
                return Ok(Some(cmd_path));
            }
        }

        // Move up to parent directory
        if let Some(parent) = current_path.parent() {
            current_path = parent.to_path_buf();
        } else {
            // Reached the root directory
            break;
        }
    }

    Ok(None)
}

/// Check if a file is executable
async fn is_executable(path: &Path) -> Result<bool> {
    match fs::metadata(path).await {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = metadata.permissions();
                Ok(permissions.mode() & 0o111 != 0)
            }
        }
        Err(_) => Ok(false),
    }
}
