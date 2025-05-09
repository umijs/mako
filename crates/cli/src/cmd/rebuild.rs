use crate::service::package::PackageService;
use anyhow::{Context, Result};

pub async fn rebuild() -> Result<()> {
    let packages = PackageService::collect_packages()
        .context("Failed to collect packages")?;

    let execution_queues = PackageService::create_execution_queues(packages)
        .context("Failed to create execution queues")?;
    PackageService::execute_queues(execution_queues)
        .await
        .context("Failed to execute queues")?;

    // Handle project's own rebuild logic
    // Since package-lock.json doesn't have root node information for "", need to manually add
    // We align with npm cli's logic: after dependency reify completes, manually execute project's own scripts
    // const scripts = [
    //     'preinstall',
    //     'install',
    //     'postinstall',
    //     'prepublish', // XXX(npm9) should we remove this finally??
    //     'preprepare',
    //     'prepare',
    //     'postprepare',
    //   ]

    PackageService::process_project_hooks()
        .await
        .context("Failed to process project hooks")
}
