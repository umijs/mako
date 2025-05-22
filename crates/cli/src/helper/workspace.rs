use anyhow::{Context, Result};
use glob::glob;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::util::{
    json::{load_package_json_from_path, read_json_file},
    logger::log_verbose,
};

pub async fn find_workspaces(root_path: &Path) -> Result<Vec<(String, PathBuf, Value)>> {
    let mut workspaces = Vec::new();
    let pkg = load_package_json_from_path(&root_path)?;

    println!("pkg: {:?}", pkg);

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
                                    let workspace_pkg = read_json_file::<Value>(&path)
                                    .context(format!(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    async fn setup_test_workspace() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path().to_path_buf();

        // Create root package.json
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

    #[tokio::test]
    async fn test_find_workspace_by_name() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let result = find_workspace_path(&root_path, "test-workspace").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "test-workspace");
    }

    #[tokio::test]
    async fn test_find_workspace_by_absolute_path() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let workspace_path = root_path.join("packages").join("test-workspace");
        let workspaces = match find_workspaces(&root_path).await {
            Ok(ws) => ws,
            Err(e) => {
                println!("Error finding workspaces: {:?}", e);
                panic!("Failed to find workspaces");
            }
        };
        let result = find_workspace_path(&root_path, &workspace_path.to_string_lossy()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), workspace_path);
    }

    #[tokio::test]
    async fn test_find_workspace_by_relative_path() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let result = find_workspace_path(&root_path, "packages/test-workspace").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().file_name().unwrap(), "test-workspace");
    }

    #[tokio::test]
    async fn test_workspace_not_found() {
        let (_temp_dir, root_path) = setup_test_workspace().await;
        let result = find_workspace_path(&root_path, "non-existent-workspace").await;
        assert!(result.is_err());
    }
}
