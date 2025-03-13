use std::path::Path;
use std::process::Command;

pub fn get_node_abi(package_path: &Path) -> Result<String, String> {
    let output = Command::new("node")
        .current_dir(package_path)
        .arg("-e")
        .arg("console.log(process.versions.modules)")
        .output()
        .map_err(|e| format!("Failed to execute node command: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let abi = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if abi.is_empty() {
        return Err("Failed to get Node.js ABI version".to_string());
    }

    Ok(format!("abi{}", abi))
}
