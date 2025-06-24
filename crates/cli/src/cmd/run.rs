use crate::helper::package::parse_package_name;
use crate::helper::workspace::{find_workspace_path, update_cwd_to_project};
use crate::model::package::{PackageInfo, Scripts};
use crate::service::script::ScriptService;
use crate::util::json::{load_package_json, load_package_json_from_path};
use crate::util::logger::log_info;
use anyhow::{Context, Result};
use serde_json::Value;

pub async fn run_script(
    script_name: &str,
    workspace: Option<&str>,
    script_args: Option<Vec<&str>>,
) -> Result<()> {
    println!(
        "script_name: {:?}, script_args: {:?}",
        script_name, script_args
    );
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    update_cwd_to_project(&cwd).await?;
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

    // Get all scripts from package.json
    let scripts = if let Some(Value::Object(scripts)) = pkg.get("scripts") {
        scripts
    } else {
        anyhow::bail!("No scripts found in package.json");
    };

    // Execute pre script if exists
    let pre_script_name = format!("pre{}", script_name);
    if let Some(Value::String(pre_script)) = scripts.get(&pre_script_name) {
        log_info(&format!("Executing pre script: {}", pre_script_name));
        ScriptService::execute_custom_script(&package, &pre_script_name, pre_script)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute pre script: {}", e))?;
    }

    // Execute main script
    let script_content = if let Some(Value::String(content)) = scripts.get(script_name) {
        content
    } else {
        anyhow::bail!("Script '{}' not found in package.json", script_name);
    };

    log_info(&format!("Executing script: {}", script_name));
    let script_args = script_args.unwrap_or_default();
    ScriptService::execute_custom_script_with_args(
        &package,
        script_name,
        script_content,
        script_args,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to execute script: {}", e))?;

    // Execute post script if exists
    let post_script_name = format!("post{}", script_name);
    if let Some(Value::String(post_script)) = scripts.get(&post_script_name) {
        log_info(&format!("Executing post script: {}", post_script_name));
        ScriptService::execute_custom_script(&package, &post_script_name, post_script)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute post script: {}", e))?;
    }

    Ok(())
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
        std::env::set_current_dir(_dir.path()).unwrap();

        let result = run_script("nonexistent", None, None).await;

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
        std::env::set_current_dir(_dir.path()).unwrap();

        let result = run_script("test", None, None).await;

        assert!(result.is_err());
    }
}
