use tokio::fs;

use crate::helper::workspace::find_workspaces;
use crate::util::logger::{log_info, log_verbose};

pub async fn clean_package_lock() -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let package_lock = current_dir.join("package-lock.json");

    if package_lock.exists() {
        fs::remove_file(&package_lock)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub async fn clean_node_modules() -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;

    // Find all workspaces
    let workspaces = find_workspaces(&current_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Clean root node_modules
    let root_node_modules = current_dir.join("node_modules");
    if root_node_modules.exists() {
        log_info("Removing root node_modules directory...");
        fs::remove_dir_all(&root_node_modules)
            .await
            .map_err(|e| e.to_string())?;
        log_verbose("Root node_modules directory removed successfully");
    }

    // Clean workspace node_modules
    for (name, path, _) in workspaces {
        let workspace_node_modules = path.join("node_modules");
        if workspace_node_modules.exists() {
            log_verbose(&format!(
                "Removing node_modules directory for workspace {}...",
                name
            ));
            fs::remove_dir_all(&workspace_node_modules)
                .await
                .map_err(|e| e.to_string())?;
            log_verbose(&format!(
                "node_modules directory removed successfully for workspace {}",
                name
            ));
        }
    }

    Ok(())
}
