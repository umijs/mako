use std::sync::Arc;

use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{Completion, Effects, OperationVc, ReadRef, ResolvedVc, Vc};
use turbopack_core::{
    diagnostics::PlainDiagnostic,
    issue::PlainIssue,
    module_graph::{GraphEntries, ModuleGraph},
    output::OutputAssets,
};

use crate::{paths::ServerPath, project::Project, utils::strongly_consistent_catch_collectables};

#[turbo_tasks::value(shared)]
#[derive(Debug, Clone)]
pub struct EndpointOutput {
    pub output_assets: ResolvedVc<OutputAssets>,
    pub project: ResolvedVc<Project>,
    pub output_paths: ResolvedVc<EndpointOutputPaths>,
}

#[turbo_tasks::value_trait]
pub trait Endpoint {
    fn output(self: Vc<Self>) -> Vc<EndpointOutput>;
    // fn write_to_disk(self: Vc<Self>) -> Vc<EndpointOutputPaths>;
    fn server_changed(self: Vc<Self>) -> Vc<Completion>;
    fn client_changed(self: Vc<Self>) -> Vc<Completion>;
    /// The entry modules for the modules graph.
    fn entries(self: Vc<Self>) -> Vc<GraphEntries>;
    /// Additional entry modules for the module graph.
    /// This may read the module graph and return additional modules.
    fn additional_entries(self: Vc<Self>, _graph: Vc<ModuleGraph>) -> Vc<GraphEntries> {
        GraphEntries::empty()
    }
}

#[turbo_tasks::value(shared)]
#[derive(Debug, Clone)]
pub enum EndpointOutputPaths {
    NodeJs {
        /// Relative to the root_path
        server_entry_path: String,
        server_paths: Vec<ServerPath>,
        client_paths: Vec<RcStr>,
    },
    Edge {
        server_paths: Vec<ServerPath>,
        client_paths: Vec<RcStr>,
    },
    // TODO: add library paths
}

#[turbo_tasks::value(transparent)]
pub struct Endpoints(pub Vec<ResolvedVc<Box<dyn Endpoint>>>);

#[turbo_tasks::function(operation)]
pub fn endpoint_server_changed_operation(
    endpoint: OperationVc<Box<dyn Endpoint>>,
) -> Vc<Completion> {
    endpoint.connect().server_changed()
}

#[turbo_tasks::function(operation)]
pub fn endpoint_write_to_disk_operation(
    endpoint: OperationVc<Box<dyn Endpoint>>,
) -> Vc<EndpointOutputPaths> {
    endpoint_write_to_disk(endpoint.connect())
}

#[turbo_tasks::function]
pub async fn endpoint_write_to_disk(
    endpoint: ResolvedVc<Box<dyn Endpoint>>,
) -> Result<Vc<EndpointOutputPaths>> {
    let output_op = output_assets_operation(endpoint);
    let EndpointOutput {
        project,
        output_paths,
        ..
    } = *output_op.connect().await?;

    let _ = project
        .emit_all_output_assets(endpoint_output_assets_operation(output_op))
        .resolve()
        .await?;

    Ok(*output_paths)
}

#[turbo_tasks::function(operation)]
fn output_assets_operation(endpoint: ResolvedVc<Box<dyn Endpoint>>) -> Vc<EndpointOutput> {
    endpoint.output()
}

#[turbo_tasks::function(operation)]
async fn endpoint_output_assets_operation(
    output: OperationVc<EndpointOutput>,
) -> Result<Vc<OutputAssets>> {
    Ok(*output.connect().await?.output_assets)
}

#[turbo_tasks::value(serialization = "none")]
pub struct WrittenEndpointWithIssues {
    pub written: Option<ReadRef<EndpointOutputPaths>>,
    pub issues: Arc<Vec<ReadRef<PlainIssue>>>,
    pub diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    pub effects: Arc<Effects>,
}

#[turbo_tasks::function(operation)]
pub async fn get_written_endpoint_with_issues_operation(
    endpoint_op: OperationVc<Box<dyn Endpoint>>,
) -> Result<Vc<WrittenEndpointWithIssues>> {
    let write_to_disk_op = endpoint_write_to_disk_operation(endpoint_op);
    let (written, issues, diagnostics, effects) =
        strongly_consistent_catch_collectables(write_to_disk_op).await?;
    Ok(WrittenEndpointWithIssues {
        written,
        issues,
        diagnostics,
        effects,
    }
    .cell())
}
