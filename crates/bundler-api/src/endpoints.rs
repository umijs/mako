use turbo_tasks::{Completion, ResolvedVc, Vc};
use turbopack_core::{
    module_graph::{GraphEntries, ModuleGraph},
    output::OutputAssets,
};

use crate::project::Project;

#[turbo_tasks::value(shared)]
#[derive(Debug, Clone)]
pub struct EndpointOutput {
    pub output_assets: ResolvedVc<OutputAssets>,
    pub project: ResolvedVc<Project>,
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

#[turbo_tasks::value(transparent)]
pub struct Endpoints(Vec<ResolvedVc<Box<dyn Endpoint>>>);
