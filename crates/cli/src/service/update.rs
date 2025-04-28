use tokio::fs;

use crate::util::logger::log_verbose;

pub async fn clean_package_lock() -> Result<(), String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let package_lock = current_dir.join("package-lock.json");
    let utoo_manifest = current_dir.join("node_modules/.utoo-manifest.json");

    if package_lock.exists() {
        fs::remove_file(&package_lock)
            .await
            .map_err(|e| e.to_string())?;
        log_verbose("package-lock.json removed successfully");
    }

    if utoo_manifest.exists() {
        fs::remove_file(&utoo_manifest)
            .await
            .map_err(|e| e.to_string())?;
        log_verbose(".utoo-manifest.json removed successfully");
    }

    Ok(())
}
