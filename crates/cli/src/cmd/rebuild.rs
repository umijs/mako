use crate::service::package::PackageService;

pub async fn rebuild() -> Result<(), String> {
    let packages = PackageService::collect_packages()?;

    let execution_queues = PackageService::create_execution_queues(packages)?;
    PackageService::execute_queues(execution_queues).await?;

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

    PackageService::process_project_hooks().await
}
