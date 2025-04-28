use std::env;
use std::fs;
use std::sync::Arc;
use std::thread;
use tokio::sync::Semaphore;

use crate::cmd::rebuild::rebuild;
use crate::helper::lock::update_package_json;
use crate::helper::lock::{ensure_package_lock, group_by_depth, PackageLock};
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
) -> Result<(), Box<dyn std::error::Error>> {
    log_verbose(&format!("update package: {:?} {:?} {:?} {:?}", action, spec, &workspace, ignore_scripts));
    // 1. Update package.json and package-lock.json
    update_package_json(&action, spec, &workspace, &save_type).await?;

    // 2. Rebuild Deps
    let _ = build_deps().await;

    install(ignore_scripts).await?;

    Ok(())
}

pub async fn install(ignore_scripts: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Package lock prerequisite check
    ensure_package_lock().await?;
    let cwd = env::current_dir()?;

    // load package-lock.json
    let package_lock: PackageLock = serde_json::from_reader(fs::File::open("package-lock.json")?)?;

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

    install_packages(&groups, &cache_dir, &cwd, semaphore).await?;

    finish_progress_bar("node_modules cloned finished");

    if !ignore_scripts {
        log_info(
            "Starting to execute dependency hook scripts, you can add --ignore-scripts to skip",
        );
        rebuild().await?;
        log_info("💫 All dependencies installed successfully");
        return Ok(());
    } else {
        log_info("💫 All dependencies installed successfully (you can run 'utoo rebuild' to trigger dependency hooks)");
        return Ok(());
    }
}
