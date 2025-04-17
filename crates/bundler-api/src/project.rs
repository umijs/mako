use std::{path::MAIN_SEPARATOR, time::Duration};

use anyhow::{bail, Context, Result};
use bundler_core::{
    all_assets_from_entries,
    client::context::{get_client_chunking_context, get_client_compile_time_info},
    config::{Config, JsConfig, ModuleIdStrategy as ModuleIdStrategyConfig},
    emit_assets,
    library::contexts::get_library_chunking_context,
    mode::Mode,
    util::Runtime,
};
use serde::{Deserialize, Serialize};
use tracing::Instrument;
use turbo_rcstr::RcStr;
use turbo_tasks::{
    graph::{AdjacencyMap, GraphTraversal},
    mark_root,
    trace::TraceRawVcs,
    Completion, Completions, IntoTraitRef, NonLocalValue, OperationValue, OperationVc, ReadRef,
    ResolvedVc, State, TaskInput, TransientInstance, TryFlatJoinIterExt, TryJoinIterExt, Vc,
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
        ChunkingContext, EvaluatableAssets, SourceMapsType,
    },
    compile_time_info::CompileTimeInfo,
    issue::{
        Issue, IssueDescriptionExt, IssueSeverity, IssueStage, OptionStyledString, StyledString,
    },
    module::Module,
    module_graph::{
        chunk_group_info::ChunkGroupEntry, GraphEntries, ModuleGraph, SingleModuleGraph,
        VisitedModules,
    },
    output::{OutputAsset, OutputAssets},
    source_map::OptionStringifiedSourceMap,
    version::{
        NotFoundVersion, OptionVersionedContent, Update, Version, VersionState, VersionedContent,
    },
    PROJECT_FILESYSTEM_NAME,
};
use turbopack_node::execution_context::ExecutionContext;
use turbopack_nodejs::NodeJsChunkingContext;

use crate::{
    endpoints::{Endpoint, Endpoints},
    entrypoints::Entrypoints,
    library::{Library, LibraryProject, OptionLibraryProject},
    versioned_content_map::VersionedContentMap,
};

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

#[derive(
    Debug,
    Default,
    Serialize,
    Deserialize,
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
pub struct EntryOptions {
    pub name: Option<RcStr>,
    pub import: RcStr,
    pub filename: Option<RcStr>,
    pub library: Option<LibraryOptions>,
}

#[derive(
    Debug,
    Default,
    Serialize,
    Deserialize,
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
pub struct LibraryOptions {
    pub name: Option<RcStr>,
    pub export: Option<Vec<RcStr>>,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Default,
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
pub struct ProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: RcStr,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: RcStr,

    /// The entrypoints of the project. Resolved relative to the project's
    /// directory (`--dir`).
    pub entry: Vec<EntryOptions>,

    /// The contents of next.config.js, serialized to JSON.
    pub config: RcStr,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: RcStr,

    /// A map of environment variables to use when compiling code.
    pub env: Vec<(RcStr, RcStr)>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: DefineEnv,

    /// Filesystem watcher options.
    pub watch: WatchOptions,

    /// The mode in which Next.js is running.
    pub dev: bool,

    /// The build id.
    pub build_id: RcStr,

    /// The browserslist query to use for targeting browsers.
    pub browserslist_query: RcStr,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    pub no_mangling: bool,
}

#[derive(
    Debug,
    Default,
    Serialize,
    Deserialize,
    Clone,
    TaskInput,
    PartialEq,
    Eq,
    Hash,
    TraceRawVcs,
    NonLocalValue,
)]
#[serde(rename_all = "camelCase")]
pub struct PartialProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: Option<RcStr>,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: Option<RcStr>,

    pub entry: Option<Vec<EntryOptions>>,

    /// The contents of next.config.js, serialized to JSON.
    pub config: Option<RcStr>,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: Option<RcStr>,

    /// A map of environment variables to use when compiling code.
    pub env: Option<Vec<(RcStr, RcStr)>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: Option<DefineEnv>,

    /// Filesystem watcher options.
    pub watch: Option<WatchOptions>,

    /// The mode in which Next.js is running.
    pub dev: Option<bool>,

    /// The build id.
    pub build_id: Option<RcStr>,

    /// The browserslist query to use for targeting browsers.
    pub browserslist_query: Option<RcStr>,

    /// The browserslist query to use for targeting browsers.
    pub no_mangling: Option<bool>,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Default,
    TaskInput,
    PartialEq,
    Eq,
    Hash,
    TraceRawVcs,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct DefineEnv {
    pub client: Vec<(RcStr, RcStr)>,
    pub edge: Vec<(RcStr, RcStr)>,
    pub nodejs: Vec<(RcStr, RcStr)>,
}

#[turbo_tasks::value]
pub struct ProjectContainer {
    name: RcStr,
    options_state: State<Option<ProjectOptions>>,
    versioned_content_map: Option<ResolvedVc<VersionedContentMap>>,
}

#[turbo_tasks::value_impl]
impl ProjectContainer {
    #[turbo_tasks::function]
    pub async fn new(name: RcStr, dev: bool) -> Result<Vc<Self>> {
        Ok(ProjectContainer {
            name,
            // we only need to enable versioning in dev mode, since build
            // is assumed to be operating over a static snapshot
            versioned_content_map: if dev {
                Some(VersionedContentMap::new())
            } else {
                None
            },
            options_state: State::new(None),
        }
        .cell())
    }
}

#[turbo_tasks::function(operation)]
fn project_fs_operation(project: ResolvedVc<Project>) -> Vc<DiskFileSystem> {
    project.project_fs()
}

#[turbo_tasks::function(operation)]
fn output_fs_operation(project: ResolvedVc<Project>) -> Vc<DiskFileSystem> {
    project.project_fs()
}

impl ProjectContainer {
    #[tracing::instrument(level = "info", name = "initialize project", skip_all)]
    pub async fn initialize(self: ResolvedVc<Self>, options: ProjectOptions) -> Result<()> {
        let watch = options.watch;

        self.await?.options_state.set(Some(options));

        let project = self.project().to_resolved().await?;
        let project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        if watch.enable {
            project_fs
                .start_watching_with_invalidation_reason(watch.poll_interval)
                .await?;
        } else {
            project_fs.invalidate_with_reason();
        }
        let output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        output_fs.invalidate_with_reason();
        Ok(())
    }

    #[tracing::instrument(level = "info", name = "update project", skip_all)]
    pub async fn update(self: Vc<Self>, options: PartialProjectOptions) -> Result<()> {
        let PartialProjectOptions {
            root_path,
            project_path,
            entry,
            config,
            js_config,
            env,
            define_env,
            watch,
            dev,
            build_id,
            browserslist_query,
            no_mangling,
        } = options;

        let this = self.await?;

        let mut new_options = this
            .options_state
            .get()
            .clone()
            .context("ProjectContainer need to be initialized with initialize()")?;

        if let Some(root_path) = root_path {
            new_options.root_path = root_path;
        }
        if let Some(project_path) = project_path {
            new_options.project_path = project_path;
        }
        if let Some(entry) = entry {
            new_options.entry = entry;
        }
        if let Some(config) = config {
            new_options.config = config;
        }
        if let Some(js_config) = js_config {
            new_options.js_config = js_config;
        }
        if let Some(env) = env {
            new_options.env = env;
        }
        if let Some(define_env) = define_env {
            new_options.define_env = define_env;
        }
        if let Some(watch) = watch {
            new_options.watch = watch;
        }
        if let Some(dev) = dev {
            new_options.dev = dev;
        }

        if let Some(build_id) = build_id {
            new_options.build_id = build_id;
        }

        if let Some(browserslist_query) = browserslist_query {
            new_options.browserslist_query = browserslist_query;
        }

        if let Some(no_mangling) = no_mangling {
            new_options.no_mangling = no_mangling;
        }

        // TODO: Handle mode switch, should prevent mode being switched.
        let watch = new_options.watch;

        let project = self.project().to_resolved().await?;
        let prev_project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        let prev_output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;

        this.options_state.set(Some(new_options));
        let project = self.project().to_resolved().await?;
        let project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        let output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;

        if !ReadRef::ptr_eq(&prev_project_fs, &project_fs) {
            if watch.enable {
                // TODO stop watching: prev_project_fs.stop_watching()?;
                project_fs
                    .start_watching_with_invalidation_reason(watch.poll_interval)
                    .await?;
            } else {
                project_fs.invalidate_with_reason();
            }
        }
        if !ReadRef::ptr_eq(&prev_output_fs, &output_fs) {
            prev_output_fs.invalidate_with_reason();
        }

        Ok(())
    }
}

#[turbo_tasks::value_impl]
impl ProjectContainer {
    #[turbo_tasks::function]
    pub async fn project(&self) -> Result<Vc<Project>> {
        let env_map: Vc<EnvMap>;
        let config;
        let define_env;
        let js_config;
        let root_path;
        let project_path;
        let entry;
        let watch;
        let build_id;
        let browserslist_query;
        let no_mangling;
        let mode;
        {
            let options = self.options_state.get();
            let options = options
                .as_ref()
                .context("ProjectContainer need to be initialized with initialize()")?;
            mode = if options.dev {
                Mode::Development
            } else {
                Mode::Build
            };
            let node_env_define: [(RcStr, RcStr); 1] = [(
                "process.env.NODE_ENV".into(),
                serde_json::to_string(mode.node_env()).unwrap().into(),
            )];
            env_map = Vc::cell(options.env.iter().cloned().collect());
            define_env = ProjectDefineEnv {
                client: ResolvedVc::cell(
                    options
                        .define_env
                        .client
                        .iter()
                        .chain(&node_env_define)
                        .cloned()
                        .collect(),
                ),
                edge: ResolvedVc::cell(
                    options
                        .define_env
                        .edge
                        .iter()
                        .chain(&node_env_define)
                        .cloned()
                        .collect(),
                ),
                nodejs: ResolvedVc::cell(
                    options
                        .define_env
                        .nodejs
                        .iter()
                        .chain(&node_env_define)
                        .cloned()
                        .collect(),
                ),
            }
            .cell();
            config = Config::from_string(Vc::cell(options.config.clone()));
            js_config = JsConfig::from_string(Vc::cell(options.js_config.clone()));
            root_path = options.root_path.clone();
            project_path = options.project_path.clone();
            entry = options.entry.clone();
            watch = options.watch;
            build_id = options.build_id.clone();
            browserslist_query = options.browserslist_query.clone();
            no_mangling = options.no_mangling
        }

        let dist_dir = config
            .await?
            .dist_dir
            .as_ref()
            .map_or_else(|| "dist".into(), |d| d.clone());

        Ok(Project {
            root_path,
            project_path,
            entry,
            watch,
            config: config.to_resolved().await?,
            js_config: js_config.to_resolved().await?,
            dist_dir,
            env: ResolvedVc::upcast(env_map.to_resolved().await?),
            define_env: define_env.to_resolved().await?,
            browserslist_query,
            mode: mode.resolved_cell(),
            versioned_content_map: self.versioned_content_map,
            build_id,
            no_mangling,
        }
        .cell())
    }

    /// See [Project::entrypoints].
    #[turbo_tasks::function]
    pub fn entrypoints(self: Vc<Self>) -> Vc<Entrypoints> {
        self.project().entrypoints()
    }

    /// See [Project::hmr_identifiers].
    #[turbo_tasks::function]
    pub fn hmr_identifiers(self: Vc<Self>) -> Vc<Vec<RcStr>> {
        self.project().hmr_identifiers()
    }

    /// Gets a source map for a particular `file_path`. If `dev` mode is disabled, this will always
    /// return [`OptionStringifiedSourceMap::none`].
    #[turbo_tasks::function]
    pub fn get_source_map(
        &self,
        file_path: Vc<FileSystemPath>,
        section: Option<RcStr>,
    ) -> Vc<OptionStringifiedSourceMap> {
        if let Some(map) = self.versioned_content_map {
            map.get_source_map(file_path, section)
        } else {
            OptionStringifiedSourceMap::none()
        }
    }
}

#[turbo_tasks::value]
pub struct Project {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    root_path: RcStr,

    /// A path where to emit the build outputs. next.config.js's distDir.
    dist_dir: RcStr,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: RcStr,

    /// The entrypoints of the project. Resolved relative to the project's
    /// directory (`--dir`).
    pub entry: Vec<EntryOptions>,

    /// Filesystem watcher options.
    watch: WatchOptions,

    /// Next config.
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

    build_id: RcStr,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    no_mangling: bool,
}

// TODO: This may be not needed.
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

#[turbo_tasks::value(shared)]
struct ConflictIssue {
    path: ResolvedVc<FileSystemPath>,
    title: ResolvedVc<StyledString>,
    description: ResolvedVc<StyledString>,
    severity: ResolvedVc<IssueSeverity>,
}

#[turbo_tasks::value_impl]
impl Issue for ConflictIssue {
    #[turbo_tasks::function]
    fn stage(&self) -> Vc<IssueStage> {
        IssueStage::AppStructure.cell()
    }

    #[turbo_tasks::function]
    fn severity(&self) -> Vc<IssueSeverity> {
        *self.severity
    }

    #[turbo_tasks::function]
    fn file_path(&self) -> Vc<FileSystemPath> {
        *self.path
    }

    #[turbo_tasks::function]
    fn title(&self) -> Vc<StyledString> {
        *self.title
    }

    #[turbo_tasks::function]
    fn description(&self) -> Vc<OptionStyledString> {
        Vc::cell(Some(self.description))
    }
}

#[turbo_tasks::value_impl]
impl Project {
    #[turbo_tasks::function]
    pub async fn library_project(self: Vc<Self>) -> Result<Vc<OptionLibraryProject>> {
        let this = self.await?;
        let lib_vec: Vec<Library> = this
            .entry
            .iter()
            .filter_map(|e| {
                e.library.as_ref().map(|l| Library {
                    name: l.name.clone(),
                    import: e.import.clone(),
                    export: l.export.clone(),
                    filename: e.filename.clone(),
                })
            })
            .collect();
        if lib_vec.is_empty() {
            Ok(Vc::cell(None))
        } else {
            Ok(Vc::cell(Some(
                LibraryProject::new(self, Vc::cell(lib_vec))
                    .to_resolved()
                    .await?,
            )))
        }
    }

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
        let config = self.config().await?;
        Ok(self
            .client_root()
            .join(format!("{}/", config.base_path.clone().unwrap_or_else(|| "".into()),).into()))
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
    pub(super) fn config(&self) -> Vc<Config> {
        *self.config
    }

    #[turbo_tasks::function]
    pub(super) fn mode(&self) -> Vc<Mode> {
        *self.mode
    }

    #[turbo_tasks::function]
    pub(super) async fn per_entry_module_graph(&self) -> Result<Vc<bool>> {
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
                node_root,
                node_root,
                node_build_environment().to_resolved().await?,
                mode.runtime_type(),
            )
            .source_maps(if *self.config().turbo_source_maps().await? {
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
    pub(super) fn client_compile_time_info(&self) -> Vc<CompileTimeInfo> {
        get_client_compile_time_info(self.browserslist_query.clone(), self.define_env.client())
    }

    #[turbo_tasks::function]
    pub async fn get_all_endpoints(self: Vc<Self>) -> Result<Vc<Endpoints>> {
        let mut endpoints = vec![];
        let entrypoints = self.entrypoints().await?;
        if let Some(libraries) = entrypoints.libraries {
            endpoints.extend(libraries.await?);
        }
        Ok(Vc::cell(endpoints))
    }

    #[turbo_tasks::function]
    pub async fn get_all_entries(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let mut modules = self
            .get_all_endpoints()
            .await?
            .iter()
            .map(async |endpoint| Ok(endpoint.entries().owned().await?))
            .try_flat_join()
            .await?;
        modules.extend(self.client_main_modules().await?.iter().cloned());
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
    ) -> Result<Vc<ModuleGraph>> {
        Ok(if *self.per_entry_module_graph().await? {
            ModuleGraph::from_entry_module(*entry)
        } else {
            *self.whole_app_module_graphs().await?.full
        })
    }

    #[turbo_tasks::function]
    pub async fn module_graph_for_modules(
        self: Vc<Self>,
        evaluatable_assets: Vc<EvaluatableAssets>,
    ) -> Result<Vc<ModuleGraph>> {
        Ok(if *self.per_entry_module_graph().await? {
            let entries = evaluatable_assets
                .await?
                .iter()
                .copied()
                .map(ResolvedVc::upcast)
                .collect();
            ModuleGraph::from_modules(Vc::cell(vec![ChunkGroupEntry::Entry(entries)]))
        } else {
            *self.whole_app_module_graphs().await?.full
        })
    }

    #[turbo_tasks::function]
    pub async fn module_graph_for_entries(
        self: Vc<Self>,
        entries: Vc<GraphEntries>,
    ) -> Result<Vc<ModuleGraph>> {
        Ok(if *self.per_entry_module_graph().await? {
            ModuleGraph::from_modules(entries)
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
        .instrument(tracing::info_span!("module graph for project"))
        .await
    }

    #[turbo_tasks::function]
    pub(super) async fn server_compile_time_info(self: Vc<Self>) -> Result<Vc<CompileTimeInfo>> {
        todo!()
    }

    #[turbo_tasks::function]
    pub(super) async fn edge_compile_time_info(self: Vc<Self>) -> Result<Vc<CompileTimeInfo>> {
        todo!()
    }

    #[turbo_tasks::function]
    pub(super) fn edge_env(&self) -> Vc<EnvMap> {
        todo!()
    }

    #[turbo_tasks::function]
    pub(super) fn client_chunking_context(self: Vc<Self>) -> Vc<Box<dyn ChunkingContext>> {
        get_client_chunking_context(
            self.project_root_path(),
            self.client_relative_path(),
            Vc::cell("/ROOT".into()),
            self.config().computed_asset_prefix(),
            self.config().chunk_suffix_path(),
            self.client_compile_time_info().environment(),
            self.mode(),
            self.module_id_strategy(),
            self.config().turbo_minify(self.mode()),
            self.config().turbo_source_maps(),
            self.no_mangling(),
        )
    }

    #[turbo_tasks::function]
    pub(super) async fn library_chunking_context(
        self: Vc<Self>,
    ) -> Result<Vc<Box<dyn ChunkingContext>>> {
        Ok(get_library_chunking_context(
            self.project_root_path(),
            self.client_relative_path(),
            Vc::cell("/ROOT".into()),
            self.config().computed_asset_prefix(),
            self.config().chunk_suffix_path(),
            self.client_compile_time_info().environment(),
            self.mode(),
            self.module_id_strategy(),
            self.config().turbo_minify(self.mode()),
            self.config().turbo_source_maps(),
            self.no_mangling(),
        ))
    }

    #[turbo_tasks::function]
    pub(super) fn server_chunking_context(
        self: Vc<Self>,
        _client_assets: bool,
    ) -> Vc<NodeJsChunkingContext> {
        todo!()
    }

    #[turbo_tasks::function]
    pub(super) fn edge_chunking_context(
        self: Vc<Self>,
        _client_assets: bool,
    ) -> Vc<Box<dyn ChunkingContext>> {
        todo!()
    }

    #[turbo_tasks::function]
    pub(super) fn runtime_chunking_context(
        self: Vc<Self>,
        client_assets: bool,
        runtime: Runtime,
    ) -> Vc<Box<dyn ChunkingContext>> {
        match runtime {
            Runtime::Edge => self.edge_chunking_context(client_assets),
            Runtime::NodeJs => Vc::upcast(self.server_chunking_context(client_assets)),
        }
    }

    #[turbo_tasks::function]
    pub async fn entrypoints(self: Vc<Self>) -> Result<Vc<Entrypoints>> {
        let library_project = self.library_project().to_resolved().await?.await?;
        Ok(Entrypoints {
            libraries: match *library_project {
                Some(lp) => {
                    let endpoints = lp
                        .get_library_endpoints()
                        .await?
                        .into_iter()
                        .map(|l| async move {
                            let endpoint: Vc<Box<dyn Endpoint>> = Vc::upcast(**l);
                            endpoint.to_resolved().await
                        })
                        .try_join()
                        .await?;
                    Some(Endpoints(endpoints.to_vec()).resolved_cell())
                }
                None => None,
            },
        }
        .cell())
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

    #[turbo_tasks::function]
    pub async fn client_main_modules(self: Vc<Self>) -> Result<Vc<GraphEntries>> {
        let modules = vec![];
        // TODO:
        Ok(Vc::cell(modules))
    }

    /// Gets the module id strategy for the project.
    #[turbo_tasks::function]
    pub async fn module_id_strategy(self: Vc<Self>) -> Result<Vc<Box<dyn ModuleIdStrategy>>> {
        let module_id_strategy =
            if let Some(module_id_strategy) = &*self.config().module_id_strategy_config().await? {
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

    let additional_module_graph =
        SingleModuleGraph::new_with_entries_visited(additional_entries, base_visited_modules);

    let full = ModuleGraph::from_graphs(vec![base_single_module_graph, additional_module_graph]);
    Ok(ModuleGraphs {
        base: base.to_resolved().await?,
        full: full.to_resolved().await?,
    }
    .cell())
}

#[turbo_tasks::value(shared)]
pub struct ModuleGraphs {
    pub base: ResolvedVc<ModuleGraph>,
    pub full: ResolvedVc<ModuleGraph>,
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

#[turbo_tasks::function(operation)]
async fn all_assets_from_entries_operation(
    operation: OperationVc<OutputAssets>,
) -> Result<Vc<OutputAssets>> {
    let assets = operation.connect();
    Ok(all_assets_from_entries(assets))
}

#[turbo_tasks::function]
fn stable_endpoint(endpoint: Vc<Box<dyn Endpoint>>) -> Vc<Box<dyn Endpoint>> {
    endpoint
}
