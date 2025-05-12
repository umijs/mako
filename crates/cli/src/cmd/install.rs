use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::sync::Arc;
use std::thread;
use tokio::sync::Semaphore;

use crate::cmd::rebuild::rebuild;
use crate::helper::lock::update_package_json;
use crate::helper::lock::{
    ensure_package_lock, group_by_depth, prepare_global_package_json, PackageLock,
};
use crate::model::package::PackageInfo;
use crate::service::install::install_packages;
use crate::util::cache::get_cache_dir;
use crate::util::logger::finish_progress_bar;
use crate::util::logger::log_verbose;
use crate::util::logger::start_progress_bar;
use crate::util::logger::{log_info, PROGRESS_BAR};
use crate::util::save_type::PackageAction;
use crate::util::save_type::SaveType;

use super::deps::build_deps;

pub async fn update_package(
    action: PackageAction,
    spec: &str,
    workspace: Option<String>,
    ignore_scripts: bool,
    save_type: SaveType,
) -> Result<()> {
    log_verbose(&format!(
        "update package: {:?} {:?} {:?} {:?}",
        action, spec, &workspace, ignore_scripts
    ));
    // 1. Update package.json and package-lock.json
    update_package_json(&action, spec, &workspace, &save_type)
        .await
        .context("Failed to update package.json")?;

    // 2. Rebuild Deps
    build_deps()
        .await
        .context("Failed to build package-lock.json")?;

    install(ignore_scripts)
        .await
        .context("Failed to install packages")?;

    Ok(())
}

pub async fn install(ignore_scripts: bool) -> Result<()> {
    // Package lock prerequisite check
    ensure_package_lock().await?;
    let cwd = env::current_dir().context("Failed to get current directory")?;

    // load package-lock.json
    let package_lock: PackageLock = serde_json::from_reader(
        fs::File::open("package-lock.json").context("Failed to open package-lock.json")?,
    )
    .map_err(|e| anyhow::anyhow!("Failed to parse package-lock.json: {}", e))?;

    let cache_dir = get_cache_dir();

    let groups = group_by_depth(&package_lock.packages);

    let mut depths: Vec<_> = groups.keys().cloned().collect();
    depths.sort_unstable();
    start_progress_bar();
    PROGRESS_BAR.set_length(package_lock.packages.len() as u64);

    // Get the number of logical CPU cores of the system and set it to twice the number of CPU cores
    let concurrent_limit = thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(20)
        .max(20);
    log_verbose(&format!("Setting concurrent limit to {}", concurrent_limit));
    let semaphore = Arc::new(Semaphore::new(concurrent_limit));

    install_packages(&groups, &cache_dir, &cwd, semaphore)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to install packages: {}", e))?;

    finish_progress_bar("node_modules cloned finished");

    if !ignore_scripts {
        log_info(
            "Starting to execute dependency hook scripts, you can add --ignore-scripts to skip",
        );
        rebuild().await.context("Failed to rebuild dependencies")?;
        log_info("💫 All dependencies installed successfully");
        Ok(())
    } else {
        log_info("💫 All dependencies installed successfully (you can run 'utoo rebuild' to trigger dependency hooks)");
        Ok(())
    }
}

pub async fn install_global_package(npm_spec: &str) -> Result<()> {
    // Prepare global package directory and package.json
    let package_path = prepare_global_package_json(npm_spec)
        .await
        .context("Failed to prepare global package.json")?;

    log_verbose(&format!("Installing global package: {}", npm_spec));

    // Change to package directory
    let original_dir = env::current_dir().context("Failed to get current directory")?;
    env::set_current_dir(&package_path).context("Failed to change to package directory")?;

    // Install dependencies
    install(false)
        .await
        .context("Failed to install global package dependencies")?;

    // Create package info from path
    let package_info =
        PackageInfo::from_path(&package_path).context("Failed to create package info from path")?;

    // Link binary files to global
    log_verbose("Linking binary files to global...");
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    package_info
        .link_to_global(
            current_exe
                .parent()
                .context("Failed to get executable parent directory")?,
        )
        .await
        .context("Failed to link binary files to global")?;

    // Change back to original directory
    env::set_current_dir(original_dir).context("Failed to change back to original directory")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_install_global_package_invalid_spec() {
        // Test installing with invalid package spec
        let result = install_global_package("invalid-package-that-does-not-exist").await;
        assert!(result.is_err(), "Should fail with invalid package spec");
    }
}
