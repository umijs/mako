use crate::cmd::install::install;
use crate::service::update::{clean_node_modules, clean_package_lock};
use crate::util::logger::log_verbose;

pub async fn update(ignore_scripts: bool) -> Result<(), String> {
    // Clean all node_modules
    // Clean package-lock.json
    log_verbose("Cleaning package-lock.json...");
    clean_package_lock().await?;

    // Clean node_modules
    log_verbose("Cleaning node_modules...");
    clean_node_modules().await?;

    // Install dependencies
    log_verbose("Installing dependencies...");
    install(ignore_scripts).await.map_err(|e| e.to_string())?;

    Ok(())
}
