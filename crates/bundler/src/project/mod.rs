use std::{path::MAIN_SEPARATOR, time::Duration};

use anyhow::{bail, Context, Result};
use entrypoint::Entrypoints;
use serde::{Deserialize, Serialize};
use tracing::Instrument;
use turbo_rcstr::RcStr;
use turbo_tasks::{
    debug::ValueDebugFormat,
    graph::{AdjacencyMap, GraphTraversal},
    mark_root,
    trace::TraceRawVcs,
    Completion, Completions, FxIndexMap, IntoTraitRef, NonLocalValue, OperationValue, OperationVc,
    ResolvedVc, TaskInput, TransientInstance, TryFlatJoinIterExt, Vc,
};
use turbo_tasks_env::{EnvMap, ProcessEnv};
use turbo_tasks_fs::{DiskFileSystem, FileSystem, FileSystemPath, VirtualFileSystem};
use turbopack::{
    evaluate_context::node_build_environment, global_module_ids::get_global_module_id_strategy,
};
use turbopack_core::{
    changed::content_changed,
    chunk::{
        module_id_strategies::{DevModuleIdStrategy, ModuleIdStrategy},
        ChunkGroupType, EvaluatableAssets, SourceMapsType,
    },
    issue::IssueDescriptionExt,
    module::Module,
    module_graph::{GraphEntries, ModuleGraph, SingleModuleGraph, VisitedModules},
    output::{OutputAsset, OutputAssets},
    version::{
        NotFoundVersion, OptionVersionedContent, Update, Version, VersionState, VersionedContent,
    },
};
use turbopack_node::execution_context::ExecutionContext;
use turbopack_nodejs::NodeJsChunkingContext;

pub mod entrypoint;
pub mod library;
pub mod project_container;
pub mod project_options;

use crate::{
    config::{
        bundle_config::Config, js_config::JsConfig, mode::Mode,
        turbo::ModuleIdStrategy as ModuleIdStrategyConfig,
    },
    emit::{all_assets_from_entries, emit_assets},
    endpoint::{AppPageRoute, Endpoint, Endpoints, Route},
    versioned_content_map::VersionedContentMap,
};

pub const PROJECT_FILESYSTEM_NAME: &str = "project";

#[turbo_tasks::value]
pub struct Project {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    root_path: RcStr,

    /// A path where to emit the build outputs. next.config.js's distDir.
    dist_dir: RcStr,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: RcStr,

    /// Filesystem watcher options.
    watch: WatchOptions,

    /// Project config.
    config: ResolvedVc<Config>,

    /// Js/Tsconfig read by load-jsconfig
    js_config: ResolvedVc<JsConfig>,

    /// A map of environment variables to use when compiling code.
    env: ResolvedVc<Box<dyn ProcessEnv>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    define_env: ResolvedVc<ProjectDefineEnv>,

    /// The browserslist query to use for targeting browsers.
    browserslist_query: RcStr,

    mode: ResolvedVc<Mode>,

    versioned_content_map: Option<ResolvedVc<VersionedContentMap>>,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    no_mangling: bool,
}

#[turbo_tasks::value_impl]
impl Project {
    #[turbo_tasks::function]
    pub fn project_fs(&self) -> Vc<DiskFileSystem> {
        DiskFileSystem::new(
            PROJECT_FILESYSTEM_NAME.into(),
            self.root_path.clone(),
            vec![],
        )
    }

    #[turbo_tasks::function]
    pub fn client_fs(self: Vc<Self>) -> Vc<Box<dyn FileSystem>> {
        let virtual_fs = VirtualFileSystem::new_with_name("client-fs".into());
        Vc::upcast(virtual_fs)
    }

    #[turbo_tasks::function]
    pub fn output_fs(&self) -> Vc<DiskFileSystem> {
        DiskFileSystem::new("output".into(), self.project_path.clone(), vec![])
    }

    #[turbo_tasks::function]
    pub fn dist_dir(&self) -> Vc<RcStr> {
        Vc::cell(self.dist_dir.clone())
    }

    #[turbo_tasks::function]
    pub async fn node_root(self: Vc<Self>) -> Result<Vc<FileSystemPath>> {
        let this = self.await?;
        Ok(self.output_fs().root().join(this.dist_dir.clone()))
    }

    #[turbo_tasks::function]
    pub fn client_root(self: Vc<Self>) -> Vc<FileSystemPath> {
        self.client_fs().root()
    }

    #[turbo_tasks::function]
    pub fn project_root_path(self: Vc<Self>) -> Vc<FileSystemPath> {
        self.project_fs().root()
    }

    #[turbo_tasks::function]
    pub async fn client_relative_path(self: Vc<Self>) -> Result<Vc<FileSystemPath>> {
        let next_config = self.bundle_config().await?;
        Ok(self.client_root().join(
            format!(
                "{}/_next",
                next_config.base_path.clone().unwrap_or_else(|| "".into()),
            )
            .into(),
        ))
    }

    #[turbo_tasks::function]
    pub async fn node_root_to_root_path(self: Vc<Self>) -> Result<Vc<RcStr>> {
        let this = self.await?;
        let output_root_to_root_path = self
            .project_path()
            .join(this.dist_dir.clone())
            .await?
            .get_relative_path_to(&*self.project_root_path().await?)
            .context("Project path need to be in root path")?;
        Ok(Vc::cell(output_root_to_root_path))
    }

    #[turbo_tasks::function]
    pub async fn project_path(self: Vc<Self>) -> Result<Vc<FileSystemPath>> {
        let this = self.await?;
        let root = self.project_root_path();
        let project_relative = this.project_path.strip_prefix(&*this.root_path).unwrap();
        let project_relative = project_relative
            .strip_prefix(MAIN_SEPARATOR)
            .unwrap_or(project_relative)
            .replace(MAIN_SEPARATOR, "/");
        Ok(root.join(project_relative.into()))
    }

    #[turbo_tasks::function]
    pub(super) fn env(&self) -> Vc<Box<dyn ProcessEnv>> {
        *self.env
    }

    #[turbo_tasks::function]
    pub(super) fn bundle_config(&self) -> Vc<Config> {
        *self.config
    }

    #[turbo_tasks::function]
    pub(super) fn mode(&self) -> Vc<Mode> {
        *self.mode
    }

    #[turbo_tasks::function]
    pub(super) async fn per_page_module_graph(&self) -> Result<Vc<bool>> {
        Ok(Vc::cell(*self.mode.await? == Mode::Development))
    }

    #[turbo_tasks::function]
    pub(super) fn js_config(&self) -> Vc<JsConfig> {
        *self.js_config
    }

    #[turbo_tasks::function]
    pub(super) fn no_mangling(&self) -> Vc<bool> {
        Vc::cell(self.no_mangling)
    }

    #[turbo_tasks::function]
    pub(super) async fn should_create_webpack_stats(&self) -> Result<Vc<bool>> {
        Ok(Vc::cell(
            self.env.read("TURBOPACK_STATS".into()).await?.is_some(),
        ))
    }

    #[turbo_tasks::function]
    pub(super) async fn execution_context(self: Vc<Self>) -> Result<Vc<ExecutionContext>> {
        let node_root = self.node_root().to_resolved().await?;
        let mode = self.mode().await?;

        let node_execution_chunking_context = Vc::upcast(
            NodeJsChunkingContext::builder(
                self.project_root_path().to_resolved().await?,
                node_root,
                self.node_root_to_root_path().to_resolved().await?,
                node_root,
                node_root.join("build/chunks".into()).to_resolved().await?,
                node_root.join("build/assets".into()).to_resolved().await?,
                node_build_environment().to_resolved().await?,
                mode.runtime_type(),
            )
            .source_maps(if *self.bundle_config().turbo_source_maps().await? {
                SourceMapsType::Full
            } else {
                SourceMapsType::None
            })
            .build(),
        );

        Ok(ExecutionContext::new(
            self.project_path(),
            node_execution_chunking_context,
            self.env(),
        ))
    }

    #[turbo_tasks::function]
    pub async fn get_all_endpoints(self: Vc<Self>) -> Result<Vc<Endpoints>> {
        let mut endpoints = Vec::new();

        let entrypoints = self.entrypoints().await?;

        for (_, route) in entrypoints.routes.iter() {
            match route {
                Route::Page {
                    html_endpoint,
                    data_endpoint: _,
                } => {
                    endpoints.push(*html_endpoint);
                }
                Route::PageApi { endpoint } => {
                    endpoints.push(*endpoint);
                }
                Route::AppPage(page_routes) => {
                    for AppPageRoute {
                        original_name: _,
                        html_endpoint,
                        rsc_endpoint: _,
                    } in page_routes
                    {
                        endpoints.push(*html_endpoint);
                    }
                }
                Route::AppRoute {
                    original_name: _,
                    endpoint,
                } => {
                    endpoints.push(*endpoint);
                }
                Route::Conflict => {
                    tracing::info!("WARN: conflict");
                }
            }
        }

        Ok(Vc::cell(endpoints))
    }

    #[turbo_tasks::function]
    pub async fn get_all_entries(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let modules = self
            .get_all_endpoints()
            .await?
            .iter()
            .map(async |endpoint| Ok(endpoint.entries().owned().await?))
            .try_flat_join()
            .await?;
        // modules.extend(self.client_main_modules().await?.iter().cloned());
        Ok(Vc::cell(modules))
    }

    #[turbo_tasks::function]
    pub async fn get_all_additional_entries(
        self: Vc<Self>,
        graphs: Vc<ModuleGraph>,
    ) -> Result<Vc<GraphEntries>> {
        let modules = self
            .get_all_endpoints()
            .await?
            .iter()
            .map(async |endpoint| Ok(endpoint.additional_entries(graphs).owned().await?))
            .try_flat_join()
            .await?;
        Ok(Vc::cell(modules))
    }

    #[turbo_tasks::function]
    pub async fn module_graph(
        self: Vc<Self>,
        entry: ResolvedVc<Box<dyn Module>>,
        chunk_group_type: ChunkGroupType,
    ) -> Result<Vc<ModuleGraph>> {
        Ok(if *self.per_page_module_graph().await? {
            ModuleGraph::from_module(*entry, chunk_group_type)
        } else {
            *self.whole_app_module_graphs().await?.full
        })
    }

    #[turbo_tasks::function]
    pub async fn module_graph_for_entries(
        self: Vc<Self>,
        evaluatable_assets: Vc<EvaluatableAssets>,
        chunk_group_type: ChunkGroupType,
    ) -> Result<Vc<ModuleGraph>> {
        Ok(if *self.per_page_module_graph().await? {
            let entries = evaluatable_assets
                .await?
                .iter()
                .copied()
                .map(ResolvedVc::upcast)
                .collect();
            ModuleGraph::from_modules(Vc::cell(vec![(entries, chunk_group_type)]))
        } else {
            *self.whole_app_module_graphs().await?.full
        })
    }

    #[turbo_tasks::function]
    pub async fn whole_app_module_graphs(self: ResolvedVc<Self>) -> Result<Vc<ModuleGraphs>> {
        async move {
            let module_graphs_op = whole_app_module_graph_operation(self);
            let module_graphs_vc = module_graphs_op.connect().resolve().await?;
            let _ = module_graphs_op.take_issues_with_path().await?;

            // At this point all modules have been computed and we can get rid of the node.js
            // process pools
            if self.await?.watch.enable {
                turbopack_node::evaluate::scale_down();
            } else {
                turbopack_node::evaluate::scale_zero();
            }

            Ok(module_graphs_vc)
        }
        .instrument(tracing::info_span!("module graph for app"))
        .await
    }

    /// Scans the app/pages directories for entry points files (matching the
    /// provided page_extensions).
    #[turbo_tasks::function]
    pub async fn entrypoints(self: Vc<Self>) -> Result<Vc<Entrypoints>> {
        let library = FxIndexMap::default();
        let routes = FxIndexMap::default();

        Ok(Entrypoints { library, routes }.cell())
    }

    #[turbo_tasks::function]
    pub async fn emit_all_output_assets(
        self: Vc<Self>,
        output_assets: OperationVc<OutputAssets>,
    ) -> Result<()> {
        let span = tracing::info_span!("emitting");
        async move {
            let all_output_assets = all_assets_from_entries_operation(output_assets);

            let client_relative_path = self.client_relative_path();
            let node_root = self.node_root();

            if let Some(map) = self.await?.versioned_content_map {
                let _ = map
                    .insert_output_assets(
                        all_output_assets,
                        node_root,
                        client_relative_path,
                        node_root,
                    )
                    .resolve()
                    .await?;

                Ok(())
            } else {
                let _ = emit_assets(
                    all_output_assets.connect(),
                    node_root,
                    client_relative_path,
                    node_root,
                )
                .resolve()
                .await?;

                Ok(())
            }
        }
        .instrument(span)
        .await
    }

    #[turbo_tasks::function]
    async fn hmr_content(self: Vc<Self>, identifier: RcStr) -> Result<Vc<OptionVersionedContent>> {
        if let Some(map) = self.await?.versioned_content_map {
            let content = map.get(self.client_relative_path().join(identifier.clone()));
            Ok(content)
        } else {
            bail!("must be in dev mode to hmr")
        }
    }

    #[turbo_tasks::function]
    async fn hmr_version(self: Vc<Self>, identifier: RcStr) -> Result<Vc<Box<dyn Version>>> {
        let content = self.hmr_content(identifier).await?;
        if let Some(content) = &*content {
            Ok(content.version())
        } else {
            Ok(Vc::upcast(NotFoundVersion::new()))
        }
    }

    /// Get the version state for a session. Initialized with the first seen
    /// version in that session.
    #[turbo_tasks::function]
    pub async fn hmr_version_state(
        self: Vc<Self>,
        identifier: RcStr,
        session: TransientInstance<()>,
    ) -> Result<Vc<VersionState>> {
        let version = self.hmr_version(identifier);

        // The session argument is important to avoid caching this function between
        // sessions.
        let _ = session;

        // INVALIDATION: This is intentionally untracked to avoid invalidating this
        // function completely. We want to initialize the VersionState with the
        // first seen version of the session.
        let state = VersionState::new(
            version
                .into_trait_ref()
                .strongly_consistent()
                .untracked()
                .await?,
        )
        .await?;
        Ok(state)
    }

    /// Emits opaque HMR events whenever a change is detected in the chunk group
    /// internally known as `identifier`.
    #[turbo_tasks::function]
    pub async fn hmr_update(
        self: Vc<Self>,
        identifier: RcStr,
        from: Vc<VersionState>,
    ) -> Result<Vc<Update>> {
        let from = from.get();
        let content = self.hmr_content(identifier).await?;
        if let Some(content) = *content {
            Ok(content.update(from))
        } else {
            Ok(Update::Missing.cell())
        }
    }

    /// Gets a list of all HMR identifiers that can be subscribed to. This is
    /// only needed for testing purposes and isn't used in real apps.
    #[turbo_tasks::function]
    pub async fn hmr_identifiers(self: Vc<Self>) -> Result<Vc<Vec<RcStr>>> {
        if let Some(map) = self.await?.versioned_content_map {
            Ok(map.keys_in_path(self.client_relative_path()))
        } else {
            bail!("must be in dev mode to hmr")
        }
    }

    /// Completion when server side changes are detected in output assets
    /// referenced from the roots
    #[turbo_tasks::function]
    pub fn server_changed(self: Vc<Self>, roots: Vc<OutputAssets>) -> Vc<Completion> {
        let path = self.node_root();
        any_output_changed(roots, path, true)
    }

    /// Completion when client side changes are detected in output assets
    /// referenced from the roots
    #[turbo_tasks::function]
    pub fn client_changed(self: Vc<Self>, roots: Vc<OutputAssets>) -> Vc<Completion> {
        let path = self.client_root();
        any_output_changed(roots, path, false)
    }

    /// Gets the module id strategy for the project.
    #[turbo_tasks::function]
    pub async fn module_id_strategy(self: Vc<Self>) -> Result<Vc<Box<dyn ModuleIdStrategy>>> {
        let module_id_strategy = if let Some(module_id_strategy) =
            &*self.bundle_config().module_id_strategy_config().await?
        {
            *module_id_strategy
        } else {
            match *self.mode().await? {
                Mode::Development => ModuleIdStrategyConfig::Named,
                Mode::Build => ModuleIdStrategyConfig::Deterministic,
            }
        };

        match module_id_strategy {
            ModuleIdStrategyConfig::Named => Ok(Vc::upcast(DevModuleIdStrategy::new())),
            ModuleIdStrategyConfig::Deterministic => {
                let module_graphs = self.whole_app_module_graphs().await?;
                Ok(Vc::upcast(get_global_module_id_strategy(
                    *module_graphs.full,
                )))
            }
        }
    }
}

#[turbo_tasks::value(shared)]
pub struct ModuleGraphs {
    pub base: ResolvedVc<ModuleGraph>,
    pub full: ResolvedVc<ModuleGraph>,
}

#[turbo_tasks::value]
pub struct ProjectDefineEnv {
    client: ResolvedVc<EnvMap>,
    edge: ResolvedVc<EnvMap>,
    nodejs: ResolvedVc<EnvMap>,
}

#[turbo_tasks::value_impl]
impl ProjectDefineEnv {
    #[turbo_tasks::function]
    pub fn client(&self) -> Vc<EnvMap> {
        *self.client
    }

    #[turbo_tasks::function]
    pub fn edge(&self) -> Vc<EnvMap> {
        *self.edge
    }

    #[turbo_tasks::function]
    pub fn nodejs(&self) -> Vc<EnvMap> {
        *self.nodejs
    }
}

#[derive(
    Debug,
    Default,
    Serialize,
    Deserialize,
    Copy,
    Clone,
    TaskInput,
    PartialEq,
    Eq,
    Hash,
    TraceRawVcs,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct WatchOptions {
    /// Whether to watch the filesystem for file changes.
    pub enable: bool,

    /// Enable polling at a certain interval if the native file watching doesn't work (e.g.
    /// docker).
    pub poll_interval: Option<Duration>,
}

#[derive(Serialize, Deserialize, TraceRawVcs, PartialEq, Eq, ValueDebugFormat, NonLocalValue)]
pub struct Middleware {
    pub endpoint: ResolvedVc<Box<dyn Endpoint>>,
}

// This is a performance optimization. This function is a root aggregation function that
// aggregates over the whole subgraph.
#[turbo_tasks::function(operation)]
async fn whole_app_module_graph_operation(
    project: ResolvedVc<Project>,
) -> Result<Vc<ModuleGraphs>> {
    mark_root();
    let base_single_module_graph = SingleModuleGraph::new_with_entries(project.get_all_entries());
    let base_visited_modules = VisitedModules::from_graph(base_single_module_graph);

    let base = ModuleGraph::from_single_graph(base_single_module_graph);
    let additional_entries = project.get_all_additional_entries(base);

    let additional_module_graph = SingleModuleGraph::new_with_entries_visited(
        additional_entries.owned().await?,
        base_visited_modules,
    );

    let full = ModuleGraph::from_graphs(vec![base_single_module_graph, additional_module_graph]);
    Ok(ModuleGraphs {
        base: base.to_resolved().await?,
        full: full.to_resolved().await?,
    }
    .cell())
}

#[turbo_tasks::function(operation)]
async fn all_assets_from_entries_operation(
    operation: OperationVc<OutputAssets>,
) -> Result<Vc<OutputAssets>> {
    let assets = operation.connect();
    Ok(all_assets_from_entries(assets))
}

#[turbo_tasks::function]
async fn any_output_changed(
    roots: Vc<OutputAssets>,
    path: Vc<FileSystemPath>,
    server: bool,
) -> Result<Vc<Completion>> {
    let path = &path.await?;
    let completions = AdjacencyMap::new()
        .skip_duplicates()
        .visit(roots.await?.iter().copied(), get_referenced_output_assets)
        .await
        .completed()?
        .into_inner()
        .into_postorder_topological()
        .map(|m| async move {
            let asset_path = m.path().await?;
            if !asset_path.path.ends_with(".map")
                && (!server || !asset_path.path.ends_with(".css"))
                && asset_path.is_inside_ref(path)
            {
                anyhow::Ok(Some(content_changed(*ResolvedVc::upcast(m))))
            } else {
                Ok(None)
            }
        })
        .map(|v| async move {
            Ok(match v.await? {
                Some(v) => Some(v.to_resolved().await?),
                None => None,
            })
        })
        .try_flat_join()
        .await?;

    Ok(Vc::<Completions>::cell(completions).completed())
}

async fn get_referenced_output_assets(
    parent: ResolvedVc<Box<dyn OutputAsset>>,
) -> Result<impl Iterator<Item = ResolvedVc<Box<dyn OutputAsset>>> + Send> {
    Ok(parent.references().owned().await?.into_iter())
}
