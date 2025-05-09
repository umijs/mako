use crate::cmd::install::install;
use crate::service::update::clean_package_lock;
use crate::util::logger::log_verbose;
use anyhow::{Context, Result};

pub async fn update(ignore_scripts: bool) -> Result<()> {
    // Clean all node_modules
    // Clean package-lock.json
    log_verbose("Cleaning package-lock.json...");
    clean_package_lock()
        .await
        .context("Failed to clean package-lock.json")?;

    // // Clean node_modules
    // log_verbose("Cleaning node_modules...");
    // clean_node_modules().await?;

    // Install dependencies
    install(ignore_scripts).await?;

    Ok(())
}
