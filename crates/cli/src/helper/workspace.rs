use anyhow::{Context, Result};
use glob::glob;
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::util::{
    json::{load_package_json_from_path, read_json_file},
    logger::{log_info, log_verbose},
};

pub async fn find_workspaces(root_path: &Path) -> Result<Vec<(String, PathBuf, Value)>> {
    let mut workspaces = Vec::new();
    let pkg = load_package_json_from_path(root_path)?;

    // load workspaces config
    if let Some(workspaces_config) = pkg.get("workspaces") {
        match workspaces_config {
            Value::Array(patterns) => {
                for pattern in patterns {
                    if let Some(pattern_str) = pattern.as_str() {
                        // prepare glob pattern
                        let package_json_path = root_path.join(pattern_str).join("package.json");
                        let glob_pattern = package_json_path.to_str().ok_or_else(|| {
                            anyhow::anyhow!(
                                "Invalid path encoding: {}",
                                package_json_path.display()
                            )
                        })?;

                        // glob match
                        for entry in glob(glob_pattern)
                            .context(format!("Invalid glob pattern: {}", glob_pattern))?
                        {
                            match entry {
                                Ok(path) => {
                                    // load package.json in workspace
                                    let workspace_pkg =
                                        read_json_file::<Value>(&path).context(format!(
                                            "Failed to parse workspace package.json at {}",
                                            path.display()
                                        ))?;

                                    // load workspace name
                                    let name =
                                        workspace_pkg["name"].as_str().unwrap_or("").to_string();

                                    // get workspace path
                                    let workspace_path = path
                                        .parent()
                                        .ok_or_else(|| {
                                            anyhow::anyhow!(
                                                "Invalid workspace path: {}",
                                                path.display()
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

pub async fn find_workspace_path(cwd: &PathBuf, workspace: &str) -> Result<PathBuf> {
    let workspaces = find_workspaces(cwd)
        .await
        .context("Failed to find workspaces")?;
    for (name, path, _) in workspaces {
        // Try exact name match
        if name == workspace {
            return Ok(path);
        }

        // Try absolute path match
        if path.to_string_lossy() == workspace {
            return Ok(path);
        }

        // Try relative path match
        if let Ok(relative) = path.strip_prefix(cwd) {
            if relative.to_string_lossy() == workspace {
                return Ok(path);
            }
        }
    }
    anyhow::bail!("Workspace '{}' not found", workspace)
}

/// Check if a directory is a workspace root by examining its package.json
async fn is_workspace_root(pkg: &Value) -> bool {
    pkg.get("workspaces").is_some()
}

/// Check if a directory is within a workspace pattern
async fn is_in_workspace(cwd: &Path, root: &Path, pattern: &str) -> Result<bool> {
    let workspace_path = root.join(pattern);
    let glob_pattern = workspace_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid path encoding: {}", workspace_path.display()))?;

    if let Ok(entries) = glob(glob_pattern) {
        for path in entries.flatten() {
            if cwd.starts_with(&path) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Find the closest directory containing package.json by traversing up
async fn find_closest_parent_pkg(start_dir: &Path) -> Result<Option<(PathBuf, Value)>> {
    let mut current = start_dir.to_path_buf();

    while let Some(parent) = current.parent() {
        let package_json_path = parent.join("package.json");
        if package_json_path.exists() {
            let pkg = read_json_file::<Value>(&package_json_path)
                .context("Failed to read package.json")?;
            return Ok(Some((parent.to_path_buf(), pkg)));
        }
        current = parent.to_path_buf();
    }

    Ok(None)
}

/// Find the project root path by traversing up the directory tree
///
/// Rules:
/// 1. If no package.json is found, return current directory as root
/// 2. If package.json is found but has no workspaces field, return its directory as root
/// 3. If package.json is found with workspaces field and current directory matches workspace pattern,
///    return the package.json directory as root
pub async fn find_root_path(force: bool) -> Result<PathBuf> {
    if !force {
        if let Some(cached) = ROOT_DIR.get() {
            return Ok(cached.clone());
        }
    }
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let (pkg_dir, pkg) = match find_closest_parent_pkg(&cwd).await? {
        Some((dir, pkg)) => (dir, pkg),
        None => return Ok(cwd),
    };
    if !is_workspace_root(&pkg).await {
        return Ok(pkg_dir);
    }
    let patterns = match pkg.get("workspaces") {
        Some(Value::Array(patterns)) => patterns,
        _ => return Ok(pkg_dir),
    };
    for pattern in patterns {
        let pattern_str = match pattern.as_str() {
            Some(s) => s,
            None => continue,
        };
        if is_in_workspace(&cwd, &pkg_dir, pattern_str).await? {
            log_verbose(&format!("Found workspace root at: {}", pkg_dir.display()));
            return Ok(pkg_dir);
        }
    }
    Ok(pkg_dir)
}

static ROOT_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Update current working directory to project root if needed
pub async fn update_cwd(force: bool) -> Result<()> {
    let root_dir = find_root_path(force).await?;
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    if !compare_paths(&current_dir, &root_dir) {
        log_info(&format!(
            "Changing directory to workspace root: {}",
            root_dir.display()
        ));
        env::set_current_dir(&root_dir).context("Failed to change to root directory")?;
    }
    if !force {
        let _ = ROOT_DIR.set(root_dir);
    }
    Ok(())
}

// Helper function to compare paths
fn compare_paths(left: &Path, right: &Path) -> bool {
    let left = left.to_string_lossy();
    let right = right.to_string_lossy();
    let left = if let Some(stripped) = left.strip_prefix("/private") {
        stripped
    } else {
        &left
    };
    let right = if let Some(stripped) = right.strip_prefix("/private") {
        stripped
    } else {
        &right
    };
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper function to normalize path for comparison
    fn normalize_path(path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        if path_str.starts_with("/private") {
            PathBuf::from(&path_str[8..])
        } else {
            path.to_path_buf()
        }
    }

    // Helper function to compare paths
    fn compare_paths(left: &Path, right: &Path) -> bool {
        let left = normalize_path(left);
        let right = normalize_path(right);
        left == right
    }

    async fn setup_test_workspace() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path().to_path_buf();

        // Create root package.json with workspaces
        let root_pkg = r#"{
            "name": "root",
            "workspaces": ["packages/*"]
        }"#;
        fs::write(root_path.join("package.json"), root_pkg).unwrap();

        // Create workspace package.json
        let workspace_dir = root_path.join("packages").join("test-workspace");
        fs::create_dir_all(&workspace_dir).unwrap();
        let workspace_pkg = r#"{
            "name": "test-workspace"
        }"#;
        fs::write(workspace_dir.join("package.json"), workspace_pkg).unwrap();

        (temp_dir, root_path)
    }

    async fn setup_test_project() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        // Create package.json without workspaces
        let pkg = r#"{
            "name": "test-project"
        }"#;
        fs::write(project_path.join("package.json"), pkg).unwrap();

        (temp_dir, project_path)
    }

    #[tokio::test]
    async fn test_find_root_path_in_workspace() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let workspace_path = root_path.join("packages").join("test-workspace");
        env::set_current_dir(&workspace_path).unwrap();
        let found_root = find_root_path(true).await.unwrap();
        assert!(compare_paths(&found_root, &root_path));
    }

    #[tokio::test]
    async fn test_find_root_path_in_root() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        env::set_current_dir(&root_path).unwrap();
        let found_root = find_root_path(true).await.unwrap();
        assert!(compare_paths(&found_root, &root_path));
    }

    #[tokio::test]
    async fn test_find_root_path_no_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().to_path_buf();
        env::set_current_dir(&test_path).unwrap();
        let found_root = find_root_path(true).await.unwrap();
        assert!(compare_paths(&found_root, &test_path));
    }

    #[tokio::test]
    async fn test_update_cwd_in_workspace() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let workspace_path = root_path.join("packages").join("test-workspace");
        env::set_current_dir(&workspace_path).unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &root_path));
        let current_dir = env::current_dir().unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &current_dir));
    }

    #[tokio::test]
    async fn test_update_cwd_in_root() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        env::set_current_dir(&root_path).unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &root_path));
        let current_dir = env::current_dir().unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &current_dir));
    }

    #[tokio::test]
    async fn test_update_cwd_in_project() {
        let (_temp_dir, project_path) = setup_test_project().await;
        env::set_current_dir(&project_path).unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &project_path));
        let current_dir = env::current_dir().unwrap();
        update_cwd(true).await.unwrap();
        assert!(compare_paths(&env::current_dir().unwrap(), &current_dir));
    }
}
