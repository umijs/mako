use std::{
    io::Write,
    path::PathBuf,
    sync::LazyLock,
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use bundler_api::{
    entrypoints::get_all_written_entrypoints_with_issues_operation,
    hmr::{
        get_hmr_identifiers_with_issues_operation, hmr_update_with_issues_operation,
        HmrIdentifiersWithIssues, HmrUpdateWithIssues,
    },
    issues::{get_entrypoints_with_issues_operation, EntrypointsWithIssues},
    operation::EntrypointsOperation,
    project::{
        DefineEnv, PartialProjectOptions, ProjectContainer, ProjectInstance, ProjectOptions,
        WatchOptions,
    },
    source_map::{get_source_map, get_source_map_rope},
    tasks::BundlerTurboTasks,
};
use bundler_core::tracing_presets::{
    TRACING_OVERVIEW_TARGETS, TRACING_TARGETS, TRACING_TURBOPACK_TARGETS,
    TRACING_TURBO_TASKS_TARGETS,
};
use napi::{
    bindgen_prelude::{within_runtime_if_available, External},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsFunction, Status,
};
use tracing::Instrument;
use tracing_subscriber::{
    fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry,
};
use turbo_rcstr::RcStr;
use turbo_tasks::{TransientInstance, UpdateInfo};
use turbo_tasks_fs::{get_relative_path_to, util::uri_from_file, FileContent, FileSystem};
use turbopack_core::{
    error::PrettyPrintError,
    source_map::Token,
    version::{PartialUpdate, TotalUpdate, Update},
    PROJECT_FILESYSTEM_NAME, SOURCE_URL_PROTOCOL,
};
use turbopack_ecmascript_hmr_protocol::{ClientUpdateInstruction, ResourceIdentifier};
use turbopack_trace_utils::{
    exit::ExitHandler, filter_layer::FilterLayer, raw_trace::RawTraceLayer,
    trace_writer::TraceWriter,
};

use super::{
    endpoint::ExternalEndpoint,
    utils::{
        create_turbo_tasks, subscribe, NapiDiagnostic, NapiIssue, RootTask, TurbopackResult, VcArc,
    },
};
use crate::{register, util::DhatProfilerGuard};

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

    /// Filesystem watcher options.
    pub watch: NapiWatchOptions,

    /// The contents of config.js, serialized to JSON.
    pub config: String,

    /// A map of environment variables to use when compiling code.
    pub process_env: Vec<NapiEnvVar>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub process_define_env: NapiDefineEnv,

    /// The mode in which Next.js is running.
    pub dev: bool,

    /// The build id.
    pub build_id: String,
}

/// [NapiProjectOptions] with all fields optional.
#[napi(object)]
pub struct NapiPartialProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: Option<String>,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: Option<String>,

    /// Filesystem watcher options.
    pub watch: Option<NapiWatchOptions>,

    /// The contents of config.js, serialized to JSON.
    pub config: Option<String>,

    /// A map of environment variables to use when compiling code.
    pub process_env: Option<Vec<NapiEnvVar>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub process_define_env: Option<NapiDefineEnv>,

    /// The mode in which Next.js is running.
    pub dev: Option<bool>,

    /// The build id.
    pub build_id: Option<String>,

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
            watch: val.watch.into(),
            config: val.config.into(),
            process_env: val
                .process_env
                .into_iter()
                .map(|var| (var.name.into(), var.value.into()))
                .collect(),
            process_define_env: val.process_define_env.into(),
            dev: val.dev,
            build_id: val.build_id.into(),
        }
    }
}

impl From<NapiPartialProjectOptions> for PartialProjectOptions {
    fn from(val: NapiPartialProjectOptions) -> Self {
        PartialProjectOptions {
            root_path: val.root_path.map(From::from),
            project_path: val.project_path.map(From::from),
            watch: val.watch.map(From::from),
            config: val.config.map(From::from),
            process_env: val.process_env.map(|env| {
                env.into_iter()
                    .map(|var| (var.name.into(), var.value.into()))
                    .collect()
            }),
            process_define_env: val.process_define_env.map(|env| env.into()),
            build_id: val.build_id.map(From::from),
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

    let mut trace = std::env::var("TURBOPACK_TRACING")
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
                trace = TRACING_OVERVIEW_TARGETS.join(",");
            }
            "bundler" => {
                trace = TRACING_TARGETS.join(",");
            }
            "turbopack" => {
                trace = TRACING_TURBOPACK_TARGETS.join(",");
            }
            "turbo-tasks" => {
                trace = TRACING_TURBO_TASKS_TARGETS.join(",");
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

        let internal_dir = PathBuf::from(&options.project_path).join(".turbopack");
        std::fs::create_dir_all(&internal_dir)
            .context("Unable to create dist directory")
            .unwrap();
        let trace_file = internal_dir.join(".trace-turbopack");
        let trace_writer = std::fs::File::create(trace_file.clone()).unwrap();
        let (trace_writer, trace_writer_guard) = TraceWriter::new(trace_writer);
        let subscriber = subscriber.with(RawTraceLayer::new(trace_writer));

        let mut tracing_server_handle: Option<JoinHandle<()>> = None;
        let trace_server = std::env::var("TURBOPACK_TRACE_SERVER").ok();
        if trace_server.is_some() {
            tracing_server_handle = Some(thread::spawn(move || {
                turbopack_trace_server::start_turbopack_trace_server(trace_file);
            }));
            println!("Turbopack trace server started. View trace at https://turbo-trace-viewer.vercel.app/");
        }

        exit.on_exit(async move {
            tokio::task::spawn_blocking(move || {
                drop(trace_writer_guard);
                if let Some(tracing_server_handle) = tracing_server_handle {
                    tracing_server_handle.join().unwrap();
                }
            })
            .await
            .unwrap();
        });

        subscriber.init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("bundler_napi=info,bundler_api=info,bundler_core=info")
            }))
            .with_timer(tracing_subscriber::fmt::time::SystemTime)
            .with_span_events(FmtSpan::CLOSE)
            .init();
    }

    let memory_limit = turbo_engine_options
        .memory_limit
        .map(|m| m as usize)
        .unwrap_or(usize::MAX);
    let persistent_caching = turbo_engine_options.persistent_caching.unwrap_or_default();
    let dependency_tracking = turbo_engine_options.dependency_tracking.unwrap_or(true);
    let turbo_tasks = create_turbo_tasks(
        PathBuf::from(&options.project_path),
        persistent_caching,
        memory_limit,
        dependency_tracking,
    )?;
    let stats_path = std::env::var_os("TURBOPACK_TASK_STATISTICS");
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
            let project = ProjectContainer::new("utoo-bundler".into(), options.dev);
            let project = project.to_resolved().await?;
            project.initialize(options).await?;
            Ok(project)
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;

    Ok(External::new_with_size_hint(
        ProjectInstance {
            turbo_tasks,
            container: *container,
            exit_receiver: tokio::sync::Mutex::new(Some(exit_receiver)),
        },
        100,
    ))
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
pub struct NapiEntrypoints {
    pub apps: Option<Vec<External<ExternalEndpoint>>>,
    pub libraries: Option<Vec<External<ExternalEndpoint>>>,
}

impl NapiEntrypoints {
    fn from_entrypoints_op(
        entrypoints: &EntrypointsOperation,
        turbo_tasks: &BundlerTurboTasks,
    ) -> Result<Self> {
        Ok(NapiEntrypoints {
            apps: entrypoints.apps.as_ref().map(|apps| {
                apps.0
                    .iter()
                    .map(|e| External::new(ExternalEndpoint(VcArc::new(turbo_tasks.clone(), *e))))
                    .collect()
            }),
            libraries: entrypoints.libraries.as_ref().map(|libraries| {
                libraries
                    .0
                    .iter()
                    .map(|e| External::new(ExternalEndpoint(VcArc::new(turbo_tasks.clone(), *e))))
                    .collect()
            }),
        })
    }
}

#[napi]
pub async fn project_write_all_entrypoints_to_disk(
    #[napi(ts_arg_type = "{ __napiType: \"Project\" }")] project: External<ProjectInstance>,
) -> napi::Result<TurbopackResult<NapiEntrypoints>> {
    let turbo_tasks = project.turbo_tasks.clone();
    let (entrypoints, issues, diags) = turbo_tasks
        .run_once(async move {
            let entrypoints_with_issues_op = get_all_written_entrypoints_with_issues_operation(
                project.container.to_resolved().await?,
            );

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
        })
        .await
        .map_err(|e| napi::Error::from_reason(PrettyPrintError(&e).to_string()))?;

    tracing::info!("all project entrypoints wrote to disk.");

    Ok(TurbopackResult {
        result: NapiEntrypoints::from_entrypoints_op(&entrypoints, &turbo_tasks)?,
        issues: issues.iter().map(|i| NapiIssue::from(&**i)).collect(),
        diagnostics: diags.iter().map(|d| NapiDiagnostic::from(d)).collect(),
    })
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
                result: NapiEntrypoints::from_entrypoints_op(&entrypoints, &turbo_tasks)?,
                issues: issues
                    .iter()
                    .map(|issue| NapiIssue::from(&**issue))
                    .collect(),
                diagnostics: diags.iter().map(|d| NapiDiagnostic::from(d)).collect(),
            }])
        },
    )
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
                uri_from_file(project.container.project().project_root(), None).await? + "/";
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
