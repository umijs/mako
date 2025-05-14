#![feature(future_join)]
#![feature(min_specialization)]

use bundler_api::project::{DefineEnv, ProjectOptions, WatchOptions};
use clap::Parser;
use dunce::canonicalize;
use serde::{Deserialize, Serialize};
use std::{fs::File, path::PathBuf, time::Instant};
use turbo_rcstr::RcStr;

use bundler_core::tracing_presets::{
    TRACING_OVERVIEW_TARGETS, TRACING_TARGETS, TRACING_TURBOPACK_TARGETS,
    TRACING_TURBO_TASKS_TARGETS,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use turbo_tasks::TurboTasks;
use turbo_tasks_backend::{noop_backing_storage, BackendOptions, TurboTasksBackend};
use turbo_tasks_malloc::TurboMalloc;
use turbopack_trace_utils::{
    exit::ExitGuard, filter_layer::FilterLayer, raw_trace::RawTraceLayer, trace_writer::TraceWriter,
};

use bundler_cli::{main_inner, Command, Mode};

#[global_allocator]
static ALLOC: TurboMalloc = TurboMalloc;

fn main() {
    let args = Command::parse();

    let dev = matches!(args.mode, Mode::Dev);

    let project_path: RcStr = canonicalize(args.project_dir)
        .unwrap()
        .to_str()
        .unwrap()
        .into();

    let root_dir = args.root_dir.map(RcStr::from);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_stop(|| {
            TurboMalloc::thread_stop();
        })
        .disable_lifo_slot()
        .build()
        .unwrap()
        .block_on(async move {
            let trace = std::env::var("TURBOPACK_TRACING").ok();

            let _guard = if let Some(mut trace) = trace.filter(|v| !v.is_empty()) {
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

                let subscriber = subscriber.with(FilterLayer::try_new(&trace).unwrap());
                let trace_file = PathBuf::from(&project_path).join("trace.log");
                let trace_writer = File::create(trace_file).unwrap();
                let (trace_writer, guard) = TraceWriter::new(trace_writer);
                let subscriber = subscriber.with(RawTraceLayer::new(trace_writer));

                let guard = ExitGuard::new(guard).unwrap();

                subscriber.init();

                Some(guard)
            } else {
                tracing_subscriber::fmt()
                    .with_env_filter(
                        EnvFilter::try_from_default_env()
                            .unwrap_or_else(|_| EnvFilter::new("bundler_cli=info")),
                    )
                    .with_timer(tracing_subscriber::fmt::time::SystemTime)
                    .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
                    .init();

                None
            };

            let turbo_tasks = TurboTasks::new(TurboTasksBackend::new(
                BackendOptions {
                    dependency_tracking: false,
                    storage_mode: None,
                    ..Default::default()
                },
                noop_backing_storage(),
            ));

            let project_options_path = PathBuf::from(&project_path).join("project_options.json");
            let mut project_options_file = File::open(&project_options_path)
                .unwrap_or_else(|_| panic!("failed to load {}", project_options_path.display()));

            let partial_project_options: PartialProjectOptions =
                serde_json::from_reader(&mut project_options_file).unwrap();
            let project_options = ProjectOptions {
                root_path: root_dir
                    .as_ref()
                    .map(|r| {
                        canonicalize(PathBuf::from(&r))
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .into()
                    })
                    .or(partial_project_options.root_path.as_ref().map(|r| {
                        canonicalize(PathBuf::from(&project_path).join(r))
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .into()
                    }))
                    .unwrap_or_else(|| project_path.clone()),
                project_path,
                config: partial_project_options
                    .config
                    .map(|c| c.to_string().into())
                    .unwrap_or(r#"{ "env": { },"experimental": { } }"#.into()),
                process_env: partial_project_options.process_env.unwrap_or_default(),
                process_define_env: partial_project_options
                    .process_define_env
                    .unwrap_or_default(),

                watch: if dev {
                    WatchOptions {
                        enable: true,
                        ..Default::default()
                    }
                } else {
                    WatchOptions {
                        enable: false,
                        ..Default::default()
                    }
                },
                build_id: partial_project_options.build_id.unwrap_or_default(),
                dev,
            };

            let result = main_inner(&turbo_tasks, project_options).await;
            let memory = TurboMalloc::memory_usage();
            tracing::info!("memory usage: {} MiB", memory / 1024 / 1024);
            let start = Instant::now();
            drop(turbo_tasks);
            tracing::info!("drop {:?}", start.elapsed());
            result
        })
        .unwrap();
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartialProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: Option<RcStr>,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: Option<RcStr>,

    /// The contents of next.config.js, serialized to JSON.
    pub config: Option<serde_json::Value>,

    /// A map of environment variables to use when compiling code.
    pub process_env: Option<Vec<(RcStr, RcStr)>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub process_define_env: Option<DefineEnv>,

    /// Filesystem watcher options.
    pub watch: Option<WatchOptions>,

    /// The build id.
    pub build_id: Option<RcStr>,
}
