use std::path::Path;
use std::process::Command;
use anyhow::{Result, Context};

pub fn get_node_abi(package_path: &Path) -> Result<String> {
    let output = Command::new("node")
        .current_dir(package_path)
        .arg("-e")
        .arg("console.log(process.versions.modules)")
        .output()
        .context("Failed to execute node command")?;

    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
    }

    let abi = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if abi.is_empty() {
        anyhow::bail!("Failed to get Node.js ABI version");
    }

    Ok(format!("abi{}", abi))
}
