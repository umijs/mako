use std::{io::Write, path::PathBuf, sync::Arc, sync::LazyLock, thread, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use bundler_api::{
    entrypoints::Entrypoints,
    operation::EntrypointsOperation,
    project::{
        DefineEnv, EntryOptions, LibraryOptions, PartialProjectOptions, Project, ProjectContainer,
        ProjectOptions, WatchOptions,
    },
};
use bundler_core::tracing_presets::{
    TRACING_BUNDLER_OVERVIEW_TARGETS, TRACING_BUNDLER_TARGETS, TRACING_BUNDLER_TURBOPACK_TARGETS,
    TRACING_BUNDLER_TURBO_TASKS_TARGETS,
};
use napi::{
    bindgen_prelude::{within_runtime_if_available, External},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsFunction, Status,
};
use rand::Rng;
use tokio::{io::AsyncWriteExt, time::Instant};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};
use turbo_rcstr::RcStr;
use turbo_tasks::{
    get_effects, Completion, Effects, ReadRef, ResolvedVc, TransientInstance, UpdateInfo, Vc,
};
use turbo_tasks_fs::{
    get_relative_path_to, util::uri_from_file, DiskFileSystem, FileContent, FileSystem,
    FileSystemPath,
};
use turbopack_core::{
    diagnostics::PlainDiagnostic,
    error::PrettyPrintError,
    issue::PlainIssue,
    source_map::{OptionSourceMap, OptionStringifiedSourceMap, SourceMap, Token},
    version::{PartialUpdate, TotalUpdate, Update, VersionState},
    PROJECT_FILESYSTEM_NAME, SOURCE_URL_PROTOCOL,
};
use turbopack_ecmascript_hmr_protocol::{ClientUpdateInstruction, ResourceIdentifier};
use turbopack_trace_utils::{
    exit::{ExitHandler, ExitReceiver},
    filter_layer::FilterLayer,
    raw_trace::RawTraceLayer,
    trace_writer::TraceWriter,
};
use url::Url;

use super::{
    endpoint::ExternalEndpoint,
    utils::{
        create_turbo_tasks, get_diagnostics, get_issues, subscribe, NapiDiagnostic, NapiIssue,
        NextTurboTasks, RootTask, TurbopackResult, VcArc,
    },
};
use crate::{register, util::DhatProfilerGuard};

/// Used by [`benchmark_file_io`]. This is a noisy benchmark, so set the
/// threshold high.
const SLOW_FILESYSTEM_THRESHOLD: Duration = Duration::from_millis(100);
static SOURCE_MAP_PREFIX: LazyLock<String> =
    LazyLock::new(|| format!("{}///", SOURCE_URL_PROTOCOL));
static SOURCE_MAP_PREFIX_PROJECT: LazyLock<String> =
    LazyLock::new(|| format!("{}///[{}]/", SOURCE_URL_PROTOCOL, PROJECT_FILESYSTEM_NAME));

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NapiEnvVar {
    pub name: String,
    pub value: String,
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NapiEntryOptions {
    pub name: Option<String>,
    pub import: String,
    pub filename: Option<String>,
    pub library: Option<NapiLibraryOptions>,
}

impl From<NapiEntryOptions> for EntryOptions {
    fn from(val: NapiEntryOptions) -> Self {
        Self {
            name: val.name.map(|n| n.into()),
            import: val.import.into(),
            filename: val.filename.map(|f| f.into()),
            library: val.library.map(|l| l.into()),
        }
    }
}

impl From<NapiLibraryOptions> for LibraryOptions {
    fn from(val: NapiLibraryOptions) -> Self {
        Self {
            name: val.name.map(|n| n.into()),
            export: val
                .export
                .map(|e| e.into_iter().map(|e| e.into()).collect()),
        }
    }
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NapiLibraryOptions {
    pub name: Option<String>,
    pub export: Option<Vec<String>>,
}

#[napi(object)]
pub struct NapiWatchOptions {
    /// Whether to watch the filesystem for file changes.
    pub enable: bool,

    /// Enable polling at a certain interval if the native file watching doesn't work (e.g.
    /// docker).
    pub poll_interval_ms: Option<f64>,
}

#[napi(object)]
pub struct NapiProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: String,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: String,

    /// next.config's distDir. Project initialization occurs earlier than
    /// deserializing next.config, so passing it as separate option.
    pub dist_dir: String,

    // Entry Config
    pub entry: Vec<NapiEntryOptions>,

    /// Filesystem watcher options.
    pub watch: NapiWatchOptions,

    /// The contents of config.js, serialized to JSON.
    pub config: String,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: String,

    /// A map of environment variables to use when compiling code.
    pub env: Vec<NapiEnvVar>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: NapiDefineEnv,

    /// The mode in which Next.js is running.
    pub dev: bool,

    /// The build id.
    pub build_id: String,

    /// The browserslist query to use for targeting browsers.
    pub browserslist_query: String,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    pub no_mangling: bool,
}

/// [NapiProjectOptions] with all fields optional.
#[napi(object)]
pub struct NapiPartialProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: Option<String>,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: Option<String>,

    // Entry Config
    pub entry: Option<Vec<NapiEntryOptions>>,

    /// next.config's distDir. Project initialization occurs earlier than
    /// deserializing next.config, so passing it as separate option.
    pub dist_dir: Option<Option<String>>,

    /// Filesystem watcher options.
    pub watch: Option<NapiWatchOptions>,

    /// The contents of config.js, serialized to JSON.
    pub config: Option<String>,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: Option<String>,

    /// A map of environment variables to use when compiling code.
    pub env: Option<Vec<NapiEnvVar>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: Option<NapiDefineEnv>,

    /// The mode in which Next.js is running.
    pub dev: Option<bool>,

    /// The build id.
    pub build_id: Option<String>,

    /// The browserslist query to use for targeting browsers.
    pub browserslist_query: Option<String>,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    pub no_mangling: Option<bool>,
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NapiDefineEnv {
    pub client: Vec<NapiEnvVar>,
    pub edge: Vec<NapiEnvVar>,
    pub nodejs: Vec<NapiEnvVar>,
}

#[napi(object)]
pub struct NapiTurboEngineOptions {
    /// Use the new backend with persistent caching enabled.
    pub persistent_caching: Option<bool>,
    /// An upper bound of memory that turbopack will attempt to stay under.
    pub memory_limit: Option<f64>,
    /// Track dependencies between tasks. If false, any change during build will error.
    pub dependency_tracking: Option<bool>,
}

impl From<NapiWatchOptions> for WatchOptions {
    fn from(val: NapiWatchOptions) -> Self {
        WatchOptions {
            enable: val.enable,
            poll_interval: val
                .poll_interval_ms
                .filter(|interval| !interval.is_nan() && interval.is_finite() && *interval > 0.0)
                .map(|interval| Duration::from_secs_f64(interval / 1000.0)),
        }
    }
}

impl From<NapiProjectOptions> for ProjectOptions {
    fn from(val: NapiProjectOptions) -> Self {
        ProjectOptions {
            root_path: val.root_path.into(),
            project_path: val.project_path.into(),
            entry: val.entry.into_iter().map(|e| e.into()).collect(),
            watch: val.watch.into(),
            config: val.config.into(),
            js_config: val.js_config.into(),
            env: val
                .env
                .into_iter()
                .map(|var| (var.name.into(), var.value.into()))
                .collect(),
            define_env: val.define_env.into(),
            dev: val.dev,
            build_id: val.build_id.into(),
            browserslist_query: val.browserslist_query.into(),
            no_mangling: val.no_mangling,
        }
    }
}

impl From<NapiPartialProjectOptions> for PartialProjectOptions {
    fn from(val: NapiPartialProjectOptions) -> Self {
        PartialProjectOptions {
            root_path: val.root_path.map(From::from),
            project_path: val.project_path.map(From::from),
            entry: val
                .entry
                .map(|entry| entry.into_iter().map(|e| e.into()).collect()),
            watch: val.watch.map(From::from),
            config: val.config.map(From::from),
            js_config: val.js_config.map(From::from),
            env: val.env.map(|env| {
                env.into_iter()
                    .map(|var| (var.name.into(), var.value.into()))
                    .collect()
            }),
            define_env: val.define_env.map(|env| env.into()),
            dev: val.dev,
            build_id: val.build_id.map(From::from),
            browserslist_query: val.browserslist_query.map(From::from),
            no_mangling: val.no_mangling,
        }
    }
}

impl From<NapiDefineEnv> for DefineEnv {
    fn from(val: NapiDefineEnv) -> Self {
        DefineEnv {
            client: val
                .client
                .into_iter()
                .map(|var| (var.name.into(), var.value.into()))
                .collect(),
            edge: val
                .edge
                .into_iter()
                .map(|var| (var.name.into(), var.value.into()))
                .collect(),
            nodejs: val
                .nodejs
                .into_iter()
                .map(|var| (var.name.into(), var.value.into()))
                .collect(),
        }
    }
}

pub struct ProjectInstance {
    turbo_tasks: NextTurboTasks,
    container: Vc<ProjectContainer>,
    exit_receiver: tokio::sync::Mutex<Option<ExitReceiver>>,
}

#[napi(ts_return_type = "Promise<{ __napiType: \"Project\" }>")]
pub async fn project_new(
    options: NapiProjectOptions,
    turbo_engine_options: NapiTurboEngineOptions,
) -> napi::Result<External<ProjectInstance>> {
    register();
    let (exit, exit_receiver) = ExitHandler::new_receiver();

    if let Some(dhat_profiler) = DhatProfilerGuard::try_init() {
        exit.on_exit(async move {
            tokio::task::spawn_blocking(move || drop(dhat_profiler))
                .await
                .unwrap()
        });
    }

    let mut trace = std::env::var("BUNDLER_TURBOPACK_TRACING")
        .ok()
        .filter(|v| !v.is_empty());

    if cfg!(feature = "tokio-console") && trace.is_none() {
        // ensure `trace` is set to *something* so that the `tokio-console` feature works, otherwise
        // you just get empty output from `tokio-console`, which can be confusing.
        trace = Some("overview".to_owned());
    }

    if let Some(mut trace) = trace {
        // Trace presets
        match trace.as_str() {
            "overview" | "1" => {
                trace = TRACING_BUNDLER_OVERVIEW_TARGETS.join(",");
            }
            "next" => {
                trace = TRACING_BUNDLER_TARGETS.join(",");
            }
            "turbopack" => {
                trace = TRACING_BUNDLER_TURBOPACK_TARGETS.join(",");
            }
            "turbo-tasks" => {
                trace = TRACING_BUNDLER_TURBO_TASKS_TARGETS.join(",");
            }
            _ => {}
        }

        let subscriber = Registry::default();

        if cfg!(feature = "tokio-console") {
            trace = format!("{trace},tokio=trace,runtime=trace");
        }
        #[cfg(feature = "tokio-console")]
        let subscriber = subscriber.with(console_subscriber::spawn());

        let subscriber = subscriber.with(FilterLayer::try_new(&trace).unwrap());
        let dist_dir = options.dist_dir.clone();

        let internal_dir = PathBuf::from(&options.project_path).join(dist_dir);
        std::fs::create_dir_all(&internal_dir)
            .context("Unable to create .next directory")
            .unwrap();
        let trace_file = internal_dir.join("trace-turbopack");
        let trace_writer = std::fs::File::create(trace_file.clone()).unwrap();
        let (trace_writer, trace_writer_guard) = TraceWriter::new(trace_writer);
        let subscriber = subscriber.with(RawTraceLayer::new(trace_writer));

        exit.on_exit(async move {
            tokio::task::spawn_blocking(move || drop(trace_writer_guard));
        });

        let trace_server = std::env::var("BUNDLER_TURBOPACK_TRACE_SERVER").ok();
        if trace_server.is_some() {
            thread::spawn(move || {
                turbopack_trace_server::start_turbopack_trace_server(trace_file);
            });
            println!("Turbopack trace server started. View trace at https://turbo-trace-viewer.vercel.app/");
        }

        subscriber.init();
    }

    let memory_limit = turbo_engine_options
        .memory_limit
        .map(|m| m as usize)
        .unwrap_or(usize::MAX);
    let persistent_caching = turbo_engine_options.persistent_caching.unwrap_or_default();
    let dependency_tracking = turbo_engine_options.dependency_tracking.unwrap_or(true);
    let turbo_tasks = create_turbo_tasks(
        PathBuf::from(&options.dist_dir),
        persistent_caching,
        memory_limit,
        dependency_tracking,
    )?;
    let stats_path = std::env::var_os("BUNDLER_TURBOPACK_TASK_STATISTICS");
    if let Some(stats_path) = stats_path {
        let task_stats = turbo_tasks.task_statistics().enable().clone();
        exit.on_exit(async move {
            tokio::task::spawn_blocking(move || {
                let mut file = std::fs::File::create(&stats_path)
                    .with_context(|| format!("failed to create or open {stats_path:?}"))?;
                serde_json::to_writer(&file, &task_stats)
                    .context("failed to serialize or write task statistics")?;
                file.flush().context("failed to flush file")
            })
            .await
            .unwrap()
            .unwrap();
        });
    }
    let options: ProjectOptions = options.into();
    let container = turbo_tasks
        .run_once(async move {
            let project = ProjectContainer::new("next.js".into(), options.dev);
            let project = project.to_resolved().await?;
            project.initialize(options).await?;
            Ok(project)
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;

    turbo_tasks.spawn_once_task(async move {
        benchmark_file_io(container.project().node_root())
            .await
            .inspect_err(|err| tracing::warn!(%err, "failed to benchmark file IO"))
    });
    Ok(External::new_with_size_hint(
        ProjectInstance {
            turbo_tasks,
            container: *container,
            exit_receiver: tokio::sync::Mutex::new(Some(exit_receiver)),
        },
        100,
    ))
}

/// A very simple and low-overhead, but potentially noisy benchmark to detect
/// very slow disk IO. Warns the user (via `println!`) if the benchmark takes
/// more than `SLOW_FILESYSTEM_THRESHOLD`.
///
/// This idea is copied from Bun:
/// - https://x.com/jarredsumner/status/1637549427677364224
/// - https://github.com/oven-sh/bun/blob/06a9aa80c38b08b3148bfeabe560/src/install/install.zig#L3038
#[tracing::instrument]
async fn benchmark_file_io(directory: Vc<FileSystemPath>) -> Result<Vc<Completion>> {
    // try to get the real file path on disk so that we can use it with tokio
    let fs = Vc::try_resolve_downcast_type::<DiskFileSystem>(directory.fs())
        .await?
        .context(anyhow!(
            "expected node_root to be a DiskFileSystem, cannot benchmark"
        ))?
        .await?;

    let directory = fs.to_sys_path(directory).await?;
    let temp_path = directory.join(format!(
        "tmp_file_io_benchmark_{:x}",
        rand::random::<u128>()
    ));

    let mut random_buffer = [0u8; 512];
    rand::rng().fill(&mut random_buffer[..]);

    // perform IO directly with tokio (skipping `tokio_tasks_fs`) to avoid the
    // additional noise/overhead of tasks caching, invalidation, file locks,
    // etc.
    let start = Instant::now();
    async move {
        for _ in 0..3 {
            // create a new empty file
            let mut file = tokio::fs::File::create(&temp_path).await?;
            file.write_all(&random_buffer).await?;
            file.sync_all().await?;
            drop(file);

            // remove the file
            tokio::fs::remove_file(&temp_path).await?;
        }
        anyhow::Ok(())
    }
    .instrument(tracing::info_span!("benchmark file IO (measurement)"))
    .await?;

    if Instant::now().duration_since(start) > SLOW_FILESYSTEM_THRESHOLD {
        println!(
            "Slow filesystem detected. If {} is a network drive, consider moving it to a local \
             folder. If you have an antivirus enabled, consider excluding your project directory.",
            directory.to_string_lossy(),
        );
    }

    Ok(Completion::new())
}

#[napi]
pub async fn project_update(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    options: NapiPartialProjectOptions,
) -> napi::Result<()> {
    let turbo_tasks = project.turbo_tasks.clone();
    let options = options.into();
    let container = project.container;
    turbo_tasks
        .run_once(async move {
            container.update(options).await?;
            Ok(())
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;
    Ok(())
}

/// Runs exit handlers for the project registered using the [`ExitHandler`] API.
///
/// This is called by `project_shutdown`, so if you're calling that API, you shouldn't call this
/// one.
#[napi]
pub async fn project_on_exit(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
) {
    project_on_exit_internal(&project).await
}

async fn project_on_exit_internal(project: &ProjectInstance) {
    let exit_receiver = project.exit_receiver.lock().await.take();
    exit_receiver
        .expect("`project.onExitSync` must only be called once")
        .run_exit_handler()
        .await;
}

/// Runs `project_on_exit`, and then waits for turbo_tasks to gracefully shut down.
///
/// This is used in builds where it's important that we completely persist turbo-tasks to disk, but
/// it's skipped in the development server (`project_on_exit` is used instead with a short timeout),
/// where we prioritize fast exit and user responsiveness over all else.
#[napi]
pub async fn project_shutdown(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
) {
    project.turbo_tasks.stop_and_wait().await;
    project_on_exit_internal(&project).await;
}

#[napi(object)]
struct NapiEntrypoints {
    pub libraries: Option<Vec<External<ExternalEndpoint>>>,
}

#[turbo_tasks::value(serialization = "none")]
struct EntrypointsWithIssues {
    entrypoints: ReadRef<EntrypointsOperation>,
    issues: Arc<Vec<ReadRef<PlainIssue>>>,
    diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    effects: Arc<Effects>,
}

#[turbo_tasks::function(operation)]
async fn get_entrypoints_with_issues_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Result<Vc<EntrypointsWithIssues>> {
    let entrypoints_operation =
        EntrypointsOperation::new(project_container_entrypoints_operation(container));
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
fn project_container_entrypoints_operation(
    // the container is a long-lived object with internally mutable state, there's no risk of it
    // becoming stale
    container: ResolvedVc<ProjectContainer>,
) -> Vc<Entrypoints> {
    container.entrypoints()
}

#[napi(ts_return_type = "{ __napiType: \"RootTask\" }")]
pub fn project_entrypoints_subscribe(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    func: JsFunction,
) -> napi::Result<External<RootTask>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let container = project.container;
    subscribe(
        turbo_tasks.clone(),
        func,
        move || {
            async move {
                let entrypoints_with_issues_op =
                    get_entrypoints_with_issues_operation(container.to_resolved().await?);
                let EntrypointsWithIssues {
                    entrypoints,
                    issues,
                    diagnostics,
                    effects,
                } = &*entrypoints_with_issues_op
                    .read_strongly_consistent()
                    .await?;
                effects.apply().await?;
                Ok((entrypoints.clone(), issues.clone(), diagnostics.clone()))
            }
            .instrument(tracing::info_span!("entrypoints subscription"))
        },
        move |ctx| {
            let (entrypoints, issues, diags) = ctx.value;

            Ok(vec![TurbopackResult {
                result: NapiEntrypoints {
                    libraries: entrypoints.libraries.as_ref().map(|libraries| {
                        libraries
                            .0
                            .iter()
                            .map(|e| {
                                External::new(ExternalEndpoint(VcArc::new(turbo_tasks.clone(), *e)))
                            })
                            .collect()
                    }),
                },
                issues: issues
                    .iter()
                    .map(|issue| NapiIssue::from(&**issue))
                    .collect(),
                diagnostics: diags.iter().map(|d| NapiDiagnostic::from(d)).collect(),
            }])
        },
    )
}

#[turbo_tasks::value(serialization = "none")]
struct HmrUpdateWithIssues {
    update: ReadRef<Update>,
    issues: Arc<Vec<ReadRef<PlainIssue>>>,
    diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    effects: Arc<Effects>,
}

#[turbo_tasks::function(operation)]
async fn hmr_update_with_issues_operation(
    project: ResolvedVc<Project>,
    identifier: RcStr,
    state: ResolvedVc<VersionState>,
) -> Result<Vc<HmrUpdateWithIssues>> {
    let update_op = project_hmr_update_operation(project, identifier, state);
    let update = update_op.read_strongly_consistent().await?;
    let issues = get_issues(update_op).await?;
    let diagnostics = get_diagnostics(update_op).await?;
    let effects = Arc::new(get_effects(update_op).await?);
    Ok(HmrUpdateWithIssues {
        update,
        issues,
        diagnostics,
        effects,
    }
    .cell())
}

#[turbo_tasks::function(operation)]
fn project_hmr_update_operation(
    project: ResolvedVc<Project>,
    identifier: RcStr,
    state: ResolvedVc<VersionState>,
) -> Vc<Update> {
    project.hmr_update(identifier, *state)
}

#[napi(ts_return_type = "{ __napiType: \"RootTask\" }")]
pub fn project_hmr_events(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    identifier: String,
    func: JsFunction,
) -> napi::Result<External<RootTask>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let project = project.container;
    let session = TransientInstance::new(());
    subscribe(
        turbo_tasks.clone(),
        func,
        {
            let outer_identifier = identifier.clone();
            let session = session.clone();
            move || {
                let identifier: RcStr = outer_identifier.clone().into();
                let session = session.clone();
                async move {
                    let project = project.project().to_resolved().await?;
                    let state = project
                        .hmr_version_state(identifier.clone(), session)
                        .to_resolved()
                        .await?;

                    let update_op =
                        hmr_update_with_issues_operation(project, identifier.clone(), state);
                    let update = update_op.read_strongly_consistent().await?;
                    let HmrUpdateWithIssues {
                        update,
                        issues,
                        diagnostics,
                        effects,
                    } = &*update;
                    effects.apply().await?;
                    match &**update {
                        Update::Missing | Update::None => {}
                        Update::Total(TotalUpdate { to }) => {
                            state.set(to.clone()).await?;
                        }
                        Update::Partial(PartialUpdate { to, .. }) => {
                            state.set(to.clone()).await?;
                        }
                    }
                    Ok((Some(update.clone()), issues.clone(), diagnostics.clone()))
                }
                .instrument(tracing::info_span!(
                    "HMR subscription",
                    identifier = outer_identifier
                ))
            }
        },
        move |ctx| {
            let (update, issues, diags) = ctx.value;

            let napi_issues = issues
                .iter()
                .map(|issue| NapiIssue::from(&**issue))
                .collect();
            let update_issues = issues
                .iter()
                .map(|issue| (&**issue).into())
                .collect::<Vec<_>>();

            let identifier = ResourceIdentifier {
                path: identifier.clone(),
                headers: None,
            };
            let update = match update.as_deref() {
                None | Some(Update::Missing) | Some(Update::Total(_)) => {
                    ClientUpdateInstruction::restart(&identifier, &update_issues)
                }
                Some(Update::Partial(update)) => ClientUpdateInstruction::partial(
                    &identifier,
                    &update.instruction,
                    &update_issues,
                ),
                Some(Update::None) => ClientUpdateInstruction::issues(&identifier, &update_issues),
            };

            Ok(vec![TurbopackResult {
                result: ctx.env.to_js_value(&update)?,
                issues: napi_issues,
                diagnostics: diags.iter().map(|d| NapiDiagnostic::from(d)).collect(),
            }])
        },
    )
}

#[napi(object)]
struct HmrIdentifiers {
    pub identifiers: Vec<String>,
}

#[turbo_tasks::value(serialization = "none")]
struct HmrIdentifiersWithIssues {
    identifiers: ReadRef<Vec<RcStr>>,
    issues: Arc<Vec<ReadRef<PlainIssue>>>,
    diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    effects: Arc<Effects>,
}

#[turbo_tasks::function(operation)]
async fn get_hmr_identifiers_with_issues_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Result<Vc<HmrIdentifiersWithIssues>> {
    let hmr_identifiers_op = project_container_hmr_identifiers_operation(container);
    let hmr_identifiers = hmr_identifiers_op.read_strongly_consistent().await?;
    let issues = get_issues(hmr_identifiers_op).await?;
    let diagnostics = get_diagnostics(hmr_identifiers_op).await?;
    let effects = Arc::new(get_effects(hmr_identifiers_op).await?);
    Ok(HmrIdentifiersWithIssues {
        identifiers: hmr_identifiers,
        issues,
        diagnostics,
        effects,
    }
    .cell())
}

#[turbo_tasks::function(operation)]
fn project_container_hmr_identifiers_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Vc<Vec<RcStr>> {
    container.hmr_identifiers()
}

#[napi(ts_return_type = "{ __napiType: \"RootTask\" }")]
pub fn project_hmr_identifiers_subscribe(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    func: JsFunction,
) -> napi::Result<External<RootTask>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let container = project.container;
    subscribe(
        turbo_tasks.clone(),
        func,
        move || async move {
            let hmr_identifiers_with_issues_op =
                get_hmr_identifiers_with_issues_operation(container.to_resolved().await?);
            let HmrIdentifiersWithIssues {
                identifiers,
                issues,
                diagnostics,
                effects,
            } = &*hmr_identifiers_with_issues_op
                .read_strongly_consistent()
                .await?;
            effects.apply().await?;

            Ok((identifiers.clone(), issues.clone(), diagnostics.clone()))
        },
        move |ctx| {
            let (identifiers, issues, diagnostics) = ctx.value;

            Ok(vec![TurbopackResult {
                result: HmrIdentifiers {
                    identifiers: identifiers
                        .iter()
                        .map(|ident| ident.to_string())
                        .collect::<Vec<_>>(),
                },
                issues: issues
                    .iter()
                    .map(|issue| NapiIssue::from(&**issue))
                    .collect(),
                diagnostics: diagnostics
                    .iter()
                    .map(|d| NapiDiagnostic::from(d))
                    .collect(),
            }])
        },
    )
}

pub enum UpdateMessage {
    Start,
    End(UpdateInfo),
}

#[napi(object)]
struct NapiUpdateMessage {
    pub update_type: String,
    pub value: Option<NapiUpdateInfo>,
}

impl From<UpdateMessage> for NapiUpdateMessage {
    fn from(update_message: UpdateMessage) -> Self {
        match update_message {
            UpdateMessage::Start => NapiUpdateMessage {
                update_type: "start".to_string(),
                value: None,
            },
            UpdateMessage::End(info) => NapiUpdateMessage {
                update_type: "end".to_string(),
                value: Some(info.into()),
            },
        }
    }
}

#[napi(object)]
struct NapiUpdateInfo {
    pub duration: u32,
    pub tasks: u32,
}

impl From<UpdateInfo> for NapiUpdateInfo {
    fn from(update_info: UpdateInfo) -> Self {
        Self {
            duration: update_info.duration.as_millis() as u32,
            tasks: update_info.tasks as u32,
        }
    }
}

/// Subscribes to lifecycle events of the compilation.
///
/// Emits an [UpdateMessage::Start] event when any computation starts.
/// Emits an [UpdateMessage::End] event when there was no computation for the
/// specified time (`aggregation_ms`). The [UpdateMessage::End] event contains
/// information about the computations that happened since the
/// [UpdateMessage::Start] event. It contains the duration of the computation
/// (excluding the idle time that was spend waiting for `aggregation_ms`), and
/// the number of tasks that were executed.
///
/// The signature of the `func` is `(update_message: UpdateMessage) => void`.
#[napi]
pub fn project_update_info_subscribe(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    aggregation_ms: u32,
    func: JsFunction,
) -> napi::Result<()> {
    let func: ThreadsafeFunction<UpdateMessage> = func.create_threadsafe_function(0, |ctx| {
        let message = ctx.value;
        Ok(vec![NapiUpdateMessage::from(message)])
    })?;
    let turbo_tasks = project.turbo_tasks.clone();
    tokio::spawn(async move {
        loop {
            let update_info = turbo_tasks
                .aggregated_update_info(Duration::ZERO, Duration::ZERO)
                .await;

            func.call(
                Ok(UpdateMessage::Start),
                ThreadsafeFunctionCallMode::NonBlocking,
            );

            let update_info = match update_info {
                Some(update_info) => update_info,
                None => {
                    turbo_tasks
                        .get_or_wait_aggregated_update_info(Duration::from_millis(
                            aggregation_ms.into(),
                        ))
                        .await
                }
            };

            let status = func.call(
                Ok(UpdateMessage::End(update_info)),
                ThreadsafeFunctionCallMode::NonBlocking,
            );

            if !matches!(status, Status::Ok) {
                let error = anyhow!("Error calling JS function: {}", status);
                eprintln!("{}", error);
                break;
            }
        }
    });
    Ok(())
}

#[turbo_tasks::value]
#[derive(Debug)]
#[napi(object)]
pub struct StackFrame {
    pub is_server: bool,
    pub is_internal: Option<bool>,
    pub original_file: Option<String>,
    pub file: String,
    // 1-indexed, unlike source map tokens
    pub line: Option<u32>,
    // 1-indexed, unlike source map tokens
    pub column: Option<u32>,
    pub method_name: Option<String>,
}

pub async fn get_source_map_rope(
    container: Vc<ProjectContainer>,
    file_path: String,
) -> Result<Option<Vc<OptionStringifiedSourceMap>>> {
    let (file, module) = match Url::parse(&file_path) {
        Ok(url) => match url.scheme() {
            "file" => {
                let path = urlencoding::decode(url.path())?.to_string();
                let module = url.query_pairs().find(|(k, _)| k == "id");
                (
                    path,
                    match module {
                        Some(module) => Some(urlencoding::decode(&module.1)?.into_owned().into()),
                        None => None,
                    },
                )
            }
            _ => bail!("Unknown url scheme"),
        },
        Err(_) => (file_path.to_string(), None),
    };

    let Some(chunk_base) = file.strip_prefix(
        &(format!(
            "{}/{}/",
            container.project().await?.project_path,
            container.project().dist_dir().await?
        )),
    ) else {
        // File doesn't exist within the dist dir
        return Ok(None);
    };

    let server_path = container.project().node_root().join(chunk_base.into());

    let client_path = container
        .project()
        .client_relative_path()
        .join(chunk_base.into());

    let mut map = container.get_source_map(server_path, module.clone());

    if map.await?.is_none() {
        // If the chunk doesn't exist as a server chunk, try a client chunk.
        // TODO: Properly tag all server chunks and use the `isServer` query param.
        // Currently, this is inaccurate as it does not cover RSC server
        // chunks.
        map = container.get_source_map(client_path, module);
    }

    if map.await?.is_none() {
        bail!("chunk/module is missing a sourcemap");
    }

    Ok(Some(map))
}

pub async fn get_source_map(
    container: Vc<ProjectContainer>,
    file_path: String,
) -> Result<Option<ReadRef<OptionSourceMap>>> {
    let Some(map) = get_source_map_rope(container, file_path).await? else {
        return Ok(None);
    };
    let map = SourceMap::new_from_rope_cached(map).await?;
    Ok(Some(map))
}

#[napi]
pub async fn project_trace_source(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    frame: StackFrame,
    current_directory_file_url: String,
) -> napi::Result<Option<StackFrame>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let container = project.container;
    let traced_frame = turbo_tasks
        .run_once(async move {
            let Some(map) = get_source_map(container, frame.file).await? else {
                return Ok(None);
            };
            let Some(map) = &*map else {
                return Ok(None);
            };

            let Some(line) = frame.line else {
                return Ok(None);
            };

            let token = map
                .lookup_token(
                    line.saturating_sub(1),
                    frame.column.unwrap_or(1).saturating_sub(1),
                )
                .await?;

            let (original_file, line, column, name) = match token {
                Token::Original(token) => (
                    urlencoding::decode(&token.original_file)?.into_owned(),
                    // JS stack frames are 1-indexed, source map tokens are 0-indexed
                    Some(token.original_line + 1),
                    Some(token.original_column + 1),
                    token.name.clone(),
                ),
                Token::Synthetic(token) => {
                    let Some(file) = &token.guessed_original_file else {
                        return Ok(None);
                    };
                    (file.to_owned(), None, None, None)
                }
            };

            let project_root_uri =
                uri_from_file(project.container.project().project_root_path(), None).await? + "/";
            let (file, original_file, is_internal) =
                if let Some(source_file) = original_file.strip_prefix(&project_root_uri) {
                    // Client code uses file://
                    (
                        get_relative_path_to(&current_directory_file_url, &original_file)
                            // TODO(sokra) remove this to include a ./ here to make it a relative path
                            .trim_start_matches("./")
                            .to_string(),
                        Some(source_file.to_string()),
                        false,
                    )
                } else if let Some(source_file) =
                    original_file.strip_prefix(&*SOURCE_MAP_PREFIX_PROJECT)
                {
                    // Server code uses turbopack:///[project]
                    // TODO should this also be file://?
                    (
                        get_relative_path_to(
                            &current_directory_file_url,
                            &format!("{}{}", project_root_uri, source_file),
                        )
                        // TODO(sokra) remove this to include a ./ here to make it a relative path
                        .trim_start_matches("./")
                        .to_string(),
                        Some(source_file.to_string()),
                        false,
                    )
                } else if let Some(source_file) = original_file.strip_prefix(&*SOURCE_MAP_PREFIX) {
                    // All other code like turbopack:///[turbopack] is internal code
                    // TODO(veil): Should the protocol be preserved?
                    (source_file.to_string(), None, true)
                } else {
                    bail!(
                        "Original file ({}) outside project ({})",
                        original_file,
                        project_root_uri
                    )
                };

            Ok(Some(StackFrame {
                file,
                original_file,
                method_name: name.as_ref().map(ToString::to_string),
                line,
                column,
                is_server: frame.is_server,
                is_internal: Some(is_internal),
            }))
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;
    Ok(traced_frame)
}

#[napi]
pub async fn project_get_source_for_asset(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    file_path: String,
) -> napi::Result<Option<String>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let source = turbo_tasks
        .run_once(async move {
            let source_content = &*project
                .container
                .project()
                .project_path()
                .fs()
                .root()
                .join(file_path.clone().into())
                .read()
                .await?;

            let FileContent::Content(source_content) = source_content else {
                bail!("Cannot find source for asset {}", file_path);
            };

            Ok(Some(source_content.content().to_str()?.into_owned()))
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;

    Ok(source)
}

#[napi]
pub async fn project_get_source_map(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    file_path: String,
) -> napi::Result<Option<String>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let container = project.container;

    let source_map = turbo_tasks
        .run_once(async move {
            let Some(map) = get_source_map_rope(container, file_path).await? else {
                return Ok(None);
            };
            let Some(map) = &*map.await? else {
                return Ok(None);
            };
            Ok(Some(map.to_str()?.to_string()))
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;

    Ok(source_map)
}

#[napi]
pub fn project_get_source_map_sync(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
    file_path: String,
) -> napi::Result<Option<String>> {
    within_runtime_if_available(|| {
        tokio::runtime::Handle::current().block_on(project_get_source_map(project, file_path))
    })
}
