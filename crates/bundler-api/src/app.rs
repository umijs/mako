use anyhow::{bail, Context, Result};
use bundler_core::client::context::{
    get_client_module_options_context, get_client_resolve_options_context,
    get_client_runtime_entries,
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
pub struct App {
    pub name: RcStr,
    pub import: RcStr,
}

#[turbo_tasks::value(transparent)]
pub struct Apps(pub Vec<App>);

#[turbo_tasks::value]
pub struct AppProject {
    pub project: ResolvedVc<Project>,
    pub apps: ResolvedVc<Apps>,
}

#[turbo_tasks::value(transparent)]
pub struct OptionAppProject(Option<ResolvedVc<AppProject>>);

#[turbo_tasks::value_impl]
impl AppProject {
    #[turbo_tasks::function]
    pub fn new(project: ResolvedVc<Project>, apps: ResolvedVc<Apps>) -> Vc<Self> {
        Self { project, apps }.cell()
    }

    #[turbo_tasks::function]
    pub fn apps(&self) -> Vc<Apps> {
        *self.apps
    }

    #[turbo_tasks::function]
    pub async fn get_app_endpoints(self: Vc<Self>) -> Result<Vc<AppEndpoints>> {
        let this = self.await?;

        let project = this.project;

        let endpoints = this
            .apps
            .await?
            .iter()
            .map(|a| async move {
                AppEndpoint {
                    project,
                    name: a.name.clone(),
                    import: a.import.clone(),
                }
                .resolved_cell()
            })
            .join()
            .await;

        Ok(AppEndpoints(endpoints).cell())
    }
}

#[turbo_tasks::value]
pub struct AppEndpoint {
    project: ResolvedVc<Project>,
    name: RcStr,
    import: RcStr,
}

#[turbo_tasks::value(transparent)]
pub struct AppEndpoints(pub Vec<ResolvedVc<AppEndpoint>>);

#[turbo_tasks::value_impl]
impl AppEndpoint {
    #[turbo_tasks::function]
    fn project(&self) -> Vc<Project> {
        *self.project
    }

    #[turbo_tasks::function]
    async fn app_module_context(self: Vc<Self>) -> Result<Vc<ModuleAssetContext>> {
        Ok(ModuleAssetContext::new(
            // FIXME:
            TransitionOptions {
                ..Default::default()
            }
            .cell(),
            self.project().client_compile_time_info(),
            self.app_module_options_context(),
            self.app_resolve_options_context(),
            Vc::cell("app".into()),
        ))
    }

    #[turbo_tasks::function]
    async fn app_module_options_context(self: Vc<Self>) -> Result<Vc<ModuleOptionsContext>> {
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
    async fn app_resolve_options_context(self: Vc<Self>) -> Result<Vc<ResolveOptionsContext>> {
        Ok(get_client_resolve_options_context(
            self.project().project_path(),
            self.project().mode(),
            self.project().config(),
            self.project().execution_context(),
        ))
    }

    #[turbo_tasks::function]
    async fn app_runtime_entries(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        Ok(get_client_runtime_entries(
            self.project().project_path(),
            self.project().mode(),
            self.project().config(),
            self.project().execution_context(),
        )
        .resolve_entries(Vc::upcast(self.app_module_context())))
    }

    #[turbo_tasks::function]
    pub async fn app_main_module(self: Vc<Self>) -> Result<Vc<Box<dyn Module>>> {
        let this = self.await?;
        let entry_request = Request::relative(
            Value::new(this.import.clone().into()),
            Default::default(),
            Default::default(),
            false,
        );

        let project_dir = self.project().await?.project_path.clone();

        let asset_context = Vc::upcast(self.app_module_context());
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
    async fn app_evaluatable_assets(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        let app_main_module = self.app_main_module();

        let Some(app_main_module) =
            Vc::try_resolve_sidecast::<Box<dyn EvaluatableAsset>>(app_main_module).await?
        else {
            bail!("expected an evaluateable asset");
        };

        let evaluatable_assets = self.app_runtime_entries().with_entry(app_main_module);

        Ok(evaluatable_assets)
    }

    #[turbo_tasks::function]
    async fn app_module_graph(self: Vc<Self>) -> Result<Vc<ModuleGraph>> {
        let project = self.project();
        let evaluatable_assets = self.app_evaluatable_assets();
        Ok(project.module_graph_for_modules(evaluatable_assets))
    }

    #[turbo_tasks::function]
    async fn app_chunk(self: Vc<Self>) -> Result<Vc<ChunkGroupResult>> {
        async move {
            let this = self.await?;

            let project = self.project();

            let app_chunking_context = project.client_chunking_context();

            let module_graph = self.app_module_graph();

            let query = QString::new(vec![("name", this.name.as_str())]).to_string();

            let app_chunk_group = app_chunking_context.evaluated_chunk_group(
                AssetIdent::from_path(project.project_root().join(this.import.clone()))
                    .with_query(Vc::cell(query.into())),
                ChunkGroup::Entry(
                    [self.app_main_module().to_resolved().await?]
                        .into_iter()
                        .collect(),
                ),
                module_graph,
                Value::new(AvailabilityInfo::Root),
            );

            Ok(app_chunk_group)
        }
        .instrument(tracing::info_span!("app chunk rendering"))
        .await
    }

    #[turbo_tasks::function]
    pub async fn output_assets(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let chunk_group_assets = *self.app_chunk().await?.assets;
        Ok(chunk_group_assets)
    }
}

#[turbo_tasks::value_impl]
impl Endpoint for AppEndpoint {
    #[turbo_tasks::function]
    async fn entries(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let mut entry_modules: Vec<ResolvedVc<Box<dyn Module>>> = self
            .app_runtime_entries()
            .await?
            .iter()
            .copied()
            .map(ResolvedVc::upcast)
            .collect();
        entry_modules.push(self.app_main_module().to_resolved().await?);
        Ok(Vc::cell(vec![ChunkGroupEntry::Entry(entry_modules)]))
    }

    #[turbo_tasks::function]
    async fn output(self: Vc<Self>) -> Result<Vc<EndpointOutput>> {
        let span = info_span!("app endpoint");
        async move {
            let this = self.await?;
            let output_assets = self.output_assets();
            let dist_root = self.project().dist_root().await?;

            let (server_paths, client_paths) = (vec![], vec![]);

            let written_endpoint = EndpointOutputPaths::NodeJs {
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
