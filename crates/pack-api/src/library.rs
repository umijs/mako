use anyhow::{bail, Context, Result};
use pack_core::{
    client::context::{
        get_client_module_options_context, get_client_resolve_options_context,
        get_client_runtime_entries,
    },
    library::contexts::get_library_chunking_context,
};
use qstring::QString;
use tracing::{info_span, Instrument};
use turbo_rcstr::RcStr;
use turbo_tasks::{Completion, JoinIterExt, ResolvedVc, Value, Vc};
use turbopack::{
    module_options::ModuleOptionsContext, resolve_options_context::ResolveOptionsContext,
    transition::TransitionOptions, ModuleAssetContext,
};
use turbopack_core::{
    chunk::{
        availability_info::AvailabilityInfo, ChunkGroupResult, ChunkingContext, EvaluatableAsset,
        EvaluatableAssets,
    },
    ident::AssetIdent,
    module::Module,
    module_graph::{
        chunk_group_info::{ChunkGroup, ChunkGroupEntry},
        GraphEntries, ModuleGraph,
    },
    output::OutputAssets,
    reference_type::{EntryReferenceSubType, ReferenceType},
    resolve::{
        origin::{PlainResolveOrigin, ResolveOriginExt},
        parse::Request,
    },
};

use crate::{
    endpoints::{Endpoint, EndpointOutput, EndpointOutputPaths},
    project::Project,
};

#[turbo_tasks::value]
pub struct Library {
    pub name: RcStr,
    pub import: RcStr,
    pub runtime_root: RcStr,
    pub runtime_export: Option<Vec<RcStr>>,
}

#[turbo_tasks::value(transparent)]
pub struct Libraries(pub Vec<Library>);

#[turbo_tasks::value]
pub struct LibraryProject {
    pub project: ResolvedVc<Project>,
    pub libraries: ResolvedVc<Libraries>,
}

#[turbo_tasks::value(transparent)]
pub struct OptionLibraryProject(Option<ResolvedVc<LibraryProject>>);

#[turbo_tasks::value_impl]
impl LibraryProject {
    #[turbo_tasks::function]
    pub fn new(project: ResolvedVc<Project>, libraries: ResolvedVc<Libraries>) -> Vc<Self> {
        Self { project, libraries }.cell()
    }

    #[turbo_tasks::function]
    pub fn libraries(&self) -> Vc<Libraries> {
        *self.libraries
    }

    #[turbo_tasks::function]
    pub async fn get_library_endpoints(self: Vc<Self>) -> Result<Vc<LibraryEndpoints>> {
        let this = self.await?;

        let project = this.project;

        let endpoints = this
            .libraries
            .await?
            .iter()
            .map(|l| async move {
                LibraryEndpoint {
                    project,
                    name: l.name.clone(),
                    import: l.import.clone(),
                    runtime_root: l.runtime_root.clone(),
                    runtime_export: l.runtime_export.clone().unwrap_or_default(),
                }
                .resolved_cell()
            })
            .join()
            .await;

        Ok(LibraryEndpoints(endpoints).cell())
    }
}

#[turbo_tasks::value]
pub struct LibraryEndpoint {
    project: ResolvedVc<Project>,
    name: RcStr,
    import: RcStr,
    runtime_root: RcStr,
    runtime_export: Vec<RcStr>,
}

#[turbo_tasks::value(transparent)]
pub struct LibraryEndpoints(pub Vec<ResolvedVc<LibraryEndpoint>>);

#[turbo_tasks::value_impl]
impl LibraryEndpoint {
    #[turbo_tasks::function]
    fn project(&self) -> Vc<Project> {
        *self.project
    }

    #[turbo_tasks::function]
    async fn library_module_context(self: Vc<Self>) -> Result<Vc<ModuleAssetContext>> {
        Ok(ModuleAssetContext::new(
            // FIXME:
            TransitionOptions {
                ..Default::default()
            }
            .cell(),
            self.project().client_compile_time_info(),
            self.library_module_options_context(),
            self.library_resolve_options_context(),
            Vc::cell("library".into()),
        ))
    }

    #[turbo_tasks::function]
    async fn library_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
        Ok(get_client_module_options_context(
            self.project().project_path(),
            self.project().execution_context(),
            self.project().client_compile_time_info().environment(),
            self.project().mode(),
            self.project().config(),
            self.project().no_mangling(),
            Vc::cell(true),
        ))
    }

    #[turbo_tasks::function]
    async fn library_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_client_resolve_options_context(
            self.project().project_path(),
            self.project().mode(),
            self.project().config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn library_runtime_entries(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        Ok(get_client_runtime_entries(
            self.project().project_path(),
            self.project().mode(),
            self.project().config(),
            self.project().execution_context(),
        )
        .resolve_entries(Vc::upcast(self.library_module_context())))
    }

    #[turbo_tasks::function]
    pub async fn library_main_module(self: Vc<Self>) -> Result<Vc<Box<dyn Module>>> {
        let this = self.await?;
        let entry_request = Request::relative(
            Value::new(this.import.clone().into()),
            Default::default(),
            Default::default(),
            false,
        );

        let project_dir = self.project().await?.project_path.clone();

        let asset_context = Vc::upcast(self.library_module_context());
        let origin = PlainResolveOrigin::new(
            asset_context,
            self.project().project_path().join("_".into()),
        );

        let entry_module = async move {
            let ty = Value::new(ReferenceType::Entry(EntryReferenceSubType::Undefined));

            let request = entry_request.await?;
            origin
                .resolve_asset(entry_request, origin.resolve_options(ty.clone()), ty)
                .await?
                .first_module()
                .await?
                .with_context(|| {
                    format!(
                        "Unable to resolve entry {} from directory {}.",
                        request.request().unwrap(),
                        project_dir
                    )
                })
        }
        .await?;

        Ok(*entry_module)
    }

    #[turbo_tasks::function]
    async fn library_evaluatable_assets(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        let library_main_module = self.library_main_module();

        let Some(library_main_module) =
            Vc::try_resolve_sidecast::<Box<dyn EvaluatableAsset>>(library_main_module).await?
        else {
            bail!("expected an evaluateable asset");
        };

        let evaluatable_assets = self
            .library_runtime_entries()
            .with_entry(library_main_module);

        Ok(evaluatable_assets)
    }

    #[turbo_tasks::function]
    async fn library_module_graph(self: Vc<Self>) -> Result<Vc<ModuleGraph>> {
        let project = self.project();
        let evaluatable_assets = self.library_evaluatable_assets();
        Ok(project.module_graph_for_modules(evaluatable_assets))
    }

    #[turbo_tasks::function]
    pub(super) async fn library_chunking_context(
        self: Vc<Self>,
        runtime_root: Vc<RcStr>,
        runtime_export: Vc<Vec<RcStr>>,
    ) -> Result<Vc<Box<dyn ChunkingContext>>> {
        let project = self.project();
        Ok(get_library_chunking_context(
            project.project_root(),
            project.dist_root(),
            Vc::cell("/ROOT".into()),
            project.client_compile_time_info().environment(),
            project.mode(),
            project.module_ids(),
            project.no_mangling(),
            runtime_root,
            runtime_export,
            project.config(),
        ))
    }

    #[turbo_tasks::function]
    async fn library_chunk(self: Vc<Self>) -> Result<Vc<ChunkGroupResult>> {
        async move {
            let this = self.await?;

            let project = self.project();

            let library_chunking_context = self.library_chunking_context(
                Vc::cell(this.runtime_root.clone()),
                Vc::cell(this.runtime_export.clone()),
            );

            let module_graph = self.library_module_graph();

            let query = QString::new(vec![("name", this.name.as_str())]).to_string();

            let library_chunk_group = library_chunking_context.evaluated_chunk_group(
                AssetIdent::from_path(project.project_root().join(this.import.clone()))
                    .with_query(Vc::cell(query.into())),
                ChunkGroup::Entry(
                    [self.library_main_module().to_resolved().await?]
                        .into_iter()
                        .collect(),
                ),
                module_graph,
                Value::new(AvailabilityInfo::Root),
            );

            Ok(library_chunk_group)
        }
        .instrument(tracing::info_span!("library chunk rendering"))
        .await
    }

    #[turbo_tasks::function]
    pub async fn output_assets(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let chunk_group_assets = *self.library_chunk().await?.assets;
        Ok(chunk_group_assets)
    }
}

#[turbo_tasks::value_impl]
impl Endpoint for LibraryEndpoint {
    #[turbo_tasks::function]
    async fn entries(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let mut entry_modules: Vec<ResolvedVc<Box<dyn Module>>> = self
            .library_runtime_entries()
            .await?
            .iter()
            .copied()
            .map(ResolvedVc::upcast)
            .collect();
        entry_modules.push(self.library_main_module().to_resolved().await?);
        Ok(Vc::cell(vec![ChunkGroupEntry::Entry(entry_modules)]))
    }

    #[turbo_tasks::function]
    async fn output(self: Vc<Self>) -> Result<Vc<EndpointOutput>> {
        let span = info_span!("library endpoint");
        async move {
            let this = self.await?;
            let output_assets = self.output_assets();
            let dist_root = self.project().dist_root().await?;

            let (server_paths, client_paths) = (vec![], vec![]);

            let written_endpoint = EndpointOutputPaths::NodeJs {
                // FIXME: No server path when bundling library
                server_entry_path: dist_root.to_string(),
                server_paths,
                client_paths,
            };

            Ok(EndpointOutput {
                output_assets: output_assets.to_resolved().await?,
                output_paths: written_endpoint.resolved_cell(),
                project: this.project,
            }
            .cell())
        }
        .instrument(span)
        .await
    }

    #[turbo_tasks::function]
    fn server_changed(self: Vc<Self>) -> Vc<Completion> {
        Completion::new()
    }

    #[turbo_tasks::function]
    fn client_changed(self: Vc<Self>) -> Vc<Completion> {
        Completion::new()
    }
}
