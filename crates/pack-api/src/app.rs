use anyhow::{bail, Result};
use futures::stream::{self, StreamExt};
use pack_core::client::context::{
    get_client_module_options_context, get_client_resolve_options_context,
    get_client_runtime_entries,
};
use qstring::QString;
use tracing::{info_span, Instrument};
use turbo_rcstr::RcStr;
use turbo_tasks::{Completion, JoinIterExt, ResolvedVc, TryJoinIterExt, Value, ValueToString, Vc};
use turbopack::{
    module_options::ModuleOptionsContext, resolve_options_context::ResolveOptionsContext,
    transition::TransitionOptions, ModuleAssetContext,
};
use turbopack_core::{
    chunk::{
        availability_info::AvailabilityInfo, ChunkGroupResult, ChunkingContext, EvaluatableAsset,
        EvaluatableAssets,
    },
    context::AssetContext,
    ident::AssetIdent,
    module::{Module, Modules},
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
    endpoint::{Endpoint, EndpointOutput, EndpointOutputPaths},
    project::Project,
};

#[turbo_tasks::value(transparent)]
pub struct AppEntripoints(pub Vec<AppEntrypoint>);

#[turbo_tasks::value]
pub struct AppProject {
    pub project: ResolvedVc<Project>,
    pub apps: ResolvedVc<AppEntripoints>,
}

#[turbo_tasks::value(transparent)]
pub struct OptionAppProject(Option<ResolvedVc<AppProject>>);

#[turbo_tasks::value_impl]
impl AppProject {
    #[turbo_tasks::function]
    pub fn new(project: ResolvedVc<Project>, apps: ResolvedVc<AppEntripoints>) -> Vc<Self> {
        Self { project, apps }.cell()
    }

    #[turbo_tasks::function]
    pub fn apps(&self) -> Vc<AppEntripoints> {
        *self.apps
    }

    #[turbo_tasks::function]
    pub async fn get_app_endpoint(self: Vc<Self>) -> Result<Vc<AppEndpoint>> {
        let this = self.await?;

        let project = this.project;

        let entrypoints = this
            .apps
            .await?
            .iter()
            .map(|a| async move {
                AppEntrypoint {
                    project,
                    name: a.name.clone(),
                    import: a.import.clone(),
                }
                .resolved_cell()
            })
            .join()
            .await;

        Ok(AppEndpoint {
            project,
            entrypoints,
        }
        .cell())
    }
}

#[turbo_tasks::value]
pub struct AppEntrypoint {
    pub project: ResolvedVc<Project>,
    pub name: RcStr,
    pub import: RcStr,
}

#[turbo_tasks::value_impl]
impl AppEntrypoint {
    #[turbo_tasks::function]
    fn project(&self) -> Vc<Project> {
        *self.project
    }

    #[turbo_tasks::function]
    pub async fn app_entry_modules(
        self: Vc<Self>,
        asset_context: Vc<Box<dyn AssetContext>>,
    ) -> Result<Vc<Modules>> {
        let this = self.await?;
        let entry_request = Request::relative(
            Value::new(this.import.clone().into()),
            Default::default(),
            Default::default(),
            false,
        );

        let origin = PlainResolveOrigin::new(
            asset_context,
            self.project().project_path().join("_".into()),
        );

        let ty = Value::new(ReferenceType::Entry(EntryReferenceSubType::Undefined));

        Ok(origin
            .resolve_asset(entry_request, origin.resolve_options(ty.clone()), ty)
            .await?
            .primary_modules())
    }

    #[turbo_tasks::function]
    pub async fn entry_evaluatable_assets(
        self: Vc<Self>,
        asset_context: Vc<Box<dyn AssetContext>>,
        runtime_entries: Vc<EvaluatableAssets>,
    ) -> Result<Vc<EvaluatableAssets>> {
        let modules = self.app_entry_modules(asset_context).await?;

        let mut all_runtime_entries = Vec::with_capacity(modules.len());
        for &module in &modules {
            if let Some(entry) = ResolvedVc::try_downcast::<Box<dyn EvaluatableAsset>>(module) {
                all_runtime_entries.push(*entry);
            } else {
                bail!(
                    "runtime reference resolved to an asset ({}) that cannot be evaluated",
                    module.ident().to_string().await?
                );
            }
        }

        all_runtime_entries.extend(runtime_entries.await?.iter().map(|e| **e));

        Ok(EvaluatableAssets::many(all_runtime_entries))
    }

    #[turbo_tasks::function]
    pub async fn module_graph_for_entry(
        self: Vc<Self>,
        asset_context: Vc<Box<dyn AssetContext>>,
        runtime_entries: Vc<EvaluatableAssets>,
    ) -> Result<Vc<ModuleGraph>> {
        let project = self.project();

        let evaluatable_asset = self.entry_evaluatable_assets(asset_context, runtime_entries);

        Ok(project.module_graph_for_modules(evaluatable_asset))
    }

    #[turbo_tasks::function]
    async fn chunk_group_for_entry(
        self: Vc<Self>,
        asset_context: Vc<Box<dyn AssetContext>>,
        runtime_entries: Vc<EvaluatableAssets>,
    ) -> Result<Vc<ChunkGroupResult>> {
        async move {
            let this = self.await?;

            let project = self.project();

            let app_chunking_context = project.client_chunking_context();

            let module_graph = self.module_graph_for_entry(asset_context, runtime_entries);

            let query = QString::new(vec![("name", this.name.as_str())]).to_string();

            let app_chunk_group = app_chunking_context.evaluated_chunk_group(
                AssetIdent::from_path(project.project_root().join(this.import.clone()))
                    .with_query(Vc::cell(query.into())),
                ChunkGroup::Entry(self.app_entry_modules(asset_context).await?.to_vec()),
                module_graph,
                Value::new(AvailabilityInfo::Root),
            );

            Ok(app_chunk_group)
        }
        .instrument(tracing::info_span!("app chunk rendering"))
        .await
    }

    #[turbo_tasks::function]
    pub async fn output_assets_for_entry(
        self: Vc<Self>,
        asset_context: Vc<Box<dyn AssetContext>>,
        runtime_entries: Vc<EvaluatableAssets>,
    ) -> Result<Vc<OutputAssets>> {
        let chunk_group_assets = *self
            .chunk_group_for_entry(asset_context, runtime_entries)
            .await?
            .assets;
        Ok(chunk_group_assets)
    }
}

#[turbo_tasks::value]
pub struct AppEndpoint {
    project: ResolvedVc<Project>,
    pub entrypoints: Vec<ResolvedVc<AppEntrypoint>>,
}

#[turbo_tasks::value_impl]
impl AppEndpoint {
    #[turbo_tasks::function]
    pub fn project(&self) -> Vc<Project> {
        *self.project
    }

    #[turbo_tasks::function]
    pub async fn app_runtime_entries(self: Vc<Self>) -> Result<Vc<EvaluatableAssets>> {
        Ok(get_client_runtime_entries(
            self.project().project_path(),
            self.project().mode(),
            self.project().config(),
            self.project().execution_context(),
            Vc::cell(self.project().await?.watch.enable),
        )
        .resolve_entries(Vc::upcast(self.app_module_context())))
    }

    #[turbo_tasks::function]
    pub async fn app_module_context(self: Vc<Self>) -> Result<Vc<ModuleAssetContext>> {
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
            Vc::cell(self.project().await?.watch.enable),
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
}

#[turbo_tasks::value_impl]
impl Endpoint for AppEndpoint {
    #[turbo_tasks::function]
    async fn entries(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let this = self.await?;

        let asset_context = self.app_module_context();

        let runtime_entries = self.app_runtime_entries();

        let entries = this
            .entrypoints
            .iter()
            .map(|e| async {
                let evaluatable_assets =
                    e.entry_evaluatable_assets(Vc::upcast(asset_context), runtime_entries);
                let entry_modules: Vec<ResolvedVc<Box<dyn Module>>> = evaluatable_assets
                    .await?
                    .iter()
                    .copied()
                    .map(ResolvedVc::upcast)
                    .collect();

                Ok(ChunkGroupEntry::Entry(entry_modules))
            })
            .try_join()
            .await?;

        Ok(Vc::cell(entries))
    }

    #[turbo_tasks::function]
    async fn output(self: Vc<Self>) -> Result<Vc<EndpointOutput>> {
        let span = info_span!("app endpoint");

        let asset_context = self.app_module_context();

        let runtime_entries = self.app_runtime_entries();

        async move {
            let this = self.await?;
            let output_assets = stream::iter(&*self.await?.entrypoints)
                .fold(OutputAssets::new(vec![]), |acc, e| async move {
                    acc.concatenate(
                        (*e).output_assets_for_entry(Vc::upcast(asset_context), runtime_entries),
                    )
                })
                .await;

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
