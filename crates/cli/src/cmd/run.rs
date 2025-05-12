use crate::helper::package::parse_package_name;
use crate::helper::workspace::find_workspace_path;
use crate::model::package::{PackageInfo, Scripts};
use crate::service::script::ScriptService;
use crate::util::logger::log_info;
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub async fn run_script(script_name: &str, workspace: Option<String>) -> Result<()> {
    let pkg = if let Some(workspace_name) = &workspace {
        let workspace_dir = find_workspace_path(
            &std::env::current_dir().context("Failed to get current directory")?,
            workspace_name,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to find workspace path: {}", e))?;
        log_info(&format!(
            "Using workspace: {} at path: {}",
            workspace_name,
            workspace_dir.display()
        ));
        load_package_json_from_path(&workspace_dir)?
    } else {
        load_package_json()?
    };

    let (scope, name, fullname) =
        parse_package_name(pkg.get("name").and_then(|v| v.as_str()).unwrap_or_default());

    let package = PackageInfo {
        path: if let Some(workspace_name) = workspace {
            find_workspace_path(
                &std::env::current_dir().context("Failed to get current directory")?,
                &workspace_name,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to find workspace path: {}", e))?
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        },
        bin_files: Default::default(),
        scripts: Scripts::default(),
        scope,
        fullname,
        name,
        version: pkg
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };

    let script_content = if let Some(Value::Object(scripts)) = pkg.get("scripts") {
        if let Some(Value::String(content)) = scripts.get(script_name) {
            content
        } else {
            anyhow::bail!("Script '{}' not found in package.json", script_name);
        }
    } else {
        anyhow::bail!("No scripts found in package.json");
    };

    log_info(&format!("Executing script: {}", script_name));
    ScriptService::execute_custom_script(&package, script_name, script_content)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to execute script: {}", e))
}

fn load_package_json() -> Result<Value> {
    let content = fs::read_to_string("package.json").context("Failed to read package.json")?;

    serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse package.json: {}", e))
}

fn load_package_json_from_path(path: &PathBuf) -> Result<Value> {
    let package_json_path = path.join("package.json");
    let content = fs::read_to_string(package_json_path).context("Failed to read package.json")?;

    serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse package.json: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_script_not_found() {
        let _dir = tempdir().unwrap();
        let package_json = r#"
        {
            "name": "@test/package",
            "version": "1.0.0",
            "scripts": {
                "test": "exit 0"
            }
        }"#;

        fs::write(_dir.path().join("package.json"), package_json).unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(_dir.path()).unwrap();

        let result = run_script("nonexistent", None).await;

        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Script 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_run_script_invalid_json() {
        let _dir = tempdir().unwrap();
        let invalid_json = r#"{ "name": "test", "scripts": { "test": 123 } }"#;

        fs::write(_dir.path().join("package.json"), invalid_json).unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(_dir.path()).unwrap();

        let result = run_script("test", None).await;

        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_err());
    }
}
