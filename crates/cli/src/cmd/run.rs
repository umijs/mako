use crate::model::package::{PackageInfo, Scripts};
use crate::service::script::ScriptService;
use crate::util::logger::log_info;
use serde_json::Value;
use std::fs;
use crate::helper::package::parse_package_name;

pub async fn run_script(script_name: &str) -> Result<(), String> {
    let pkg = load_package_json()?;

    // 解析包名
    let (scope, name, fullname) = parse_package_name(
        pkg.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
    );

    // 创建 PackageInfo 实例
    let package = PackageInfo {
        path: std::env::current_dir().map_err(|e| e.to_string())?,
        bin_files: Default::default(),
        scripts: Scripts::default(),
        scope,
        fullname,
        name,
        version: pkg.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };

    // 获取脚本内容
    let script_content = if let Some(Value::Object(scripts)) = pkg.get("scripts") {
        if let Some(Value::String(content)) = scripts.get(script_name) {
            content
        } else {
            return Err(format!("Script '{}' not found in package.json", script_name));
        }
    } else {
        return Err("No scripts found in package.json".to_string());
    };

    log_info(&format!("Executing script: {}", script_name));
    ScriptService::execute_custom_script(&package, script_name, script_content, true).await
}

fn load_package_json() -> Result<Value, String> {
    fs::read_to_string("package.json")
        .map_err(|e| format!("Failed to read package.json: {}", e))
        .and_then(|content| {
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse package.json: {}", e))
        })
}
