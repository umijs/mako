use anyhow::Result;
use std::sync::Arc;
use turbo_tasks::{get_effects, FxIndexSet, ResolvedVc, TryJoinIterExt, Vc};
use turbopack_core::output::{OutputAsset, OutputAssets};

use crate::{
    endpoints::{Endpoint, Endpoints},
    issues::{get_diagnostics, get_issues, EntrypointsWithIssues},
    operation::EntrypointsOperation,
    project::ProjectContainer,
};

#[turbo_tasks::value(shared)]
pub struct Entrypoints {
    pub apps: Option<ResolvedVc<Endpoints>>,
    pub libraries: Option<ResolvedVc<Endpoints>>,
}

#[turbo_tasks::function(operation)]
pub async fn get_all_written_entrypoints_with_issues_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Result<Vc<EntrypointsWithIssues>> {
    let entrypoints_operation =
        EntrypointsOperation::new(all_entrypoints_write_to_disk_operation(container));
    let entrypoints = entrypoints_operation.read_strongly_consistent().await?;
    let issues = get_issues(entrypoints_operation).await?;
    let diagnostics = get_diagnostics(entrypoints_operation).await?;
    let effects = Arc::new(get_effects(entrypoints_operation).await?);
    Ok(EntrypointsWithIssues {
        entrypoints,
        issues,
        diagnostics,
        effects,
    }
    .cell())
}

#[turbo_tasks::function(operation)]
pub async fn all_entrypoints_write_to_disk_operation(
    project: ResolvedVc<ProjectContainer>,
) -> Result<Vc<Entrypoints>> {
    let _ = project
        .project()
        .emit_all_output_assets(all_output_assets_operation(project))
        .resolve()
        .await?;

    Ok(project.entrypoints())
}

#[turbo_tasks::function(operation)]
pub async fn all_output_assets_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Result<Vc<OutputAssets>> {
    let endpoint_assets = container
        .project()
        .get_all_endpoints()
        .await?
        .iter()
        .map(|endpoint| async move { endpoint.output().await?.output_assets.await })
        .try_join()
        .await?;

    let mut output_assets: FxIndexSet<ResolvedVc<Box<dyn OutputAsset>>> = FxIndexSet::default();
    for assets in endpoint_assets {
        output_assets.extend(assets.iter());
    }

    Ok(Vc::cell(output_assets.into_iter().collect()))
}
