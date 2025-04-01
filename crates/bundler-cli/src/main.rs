#![feature(future_join)]
#![feature(min_specialization)]

use bundler_api::project::{PartialProjectOptions, ProjectOptions, WatchOptions};
use clap::Parser;
use dunce::canonicalize;
use std::{env::current_dir, fs::File, io::Read, path::PathBuf, time::Instant};
use turbo_rcstr::RcStr;

use bundler_core::tracing_presets::{
    TRACING_BUNDLER_OVERVIEW_TARGETS, TRACING_BUNDLER_TARGETS, TRACING_BUNDLER_TURBOPACK_TARGETS,
    TRACING_BUNDLER_TURBO_TASKS_TARGETS,
};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use turbo_tasks::TurboTasks;
use turbo_tasks_backend::{noop_backing_storage, BackendOptions, TurboTasksBackend};
use turbo_tasks_malloc::TurboMalloc;
use turbopack_trace_utils::{
    filter_layer::FilterLayer,
    exit::ExitGuard,raw_trace::RawTraceLayer, trace_writer::TraceWriter,
};

use bundler_cli::{args::CliArgs, main_inner};

#[global_allocator]
static ALLOC: TurboMalloc = TurboMalloc;

fn main() {
    let args = CliArgs::parse();

    let dev;
    let options;
    {
        match args {
            CliArgs::Build(opts) => {
                dev = false;
                options = opts
            }
            CliArgs::Dev(opts) => {
                dev = true;
                options = opts
            }
        }
    }

    let project_path: RcStr = options
        .project
        .as_ref()
        .map(canonicalize)
        .unwrap_or_else(current_dir)
        .expect("project directory can't be found")
        .to_str()
        .expect("project directory contains invalid characters")
        .into();

    let root_path = match options.root.as_ref() {
        Some(root) => canonicalize(root)
            .expect("root directory can't be found")
            .to_str()
            .expect("root directory contains invalid characters")
            .into(),
        None => project_path.clone(),
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_stop(|| {
            TurboMalloc::thread_stop();
        })
        .disable_lifo_slot()
        .build()
        .unwrap()
        .block_on(async move {
            let trace = std::env::var("BUNDLER_TURBOPACK_TRACING").ok();

            let _guard = if let Some(mut trace) = trace.filter(|v| !v.is_empty()) {
                // Trace presets
                match trace.as_str() {
                    "overview" | "1" => {
                        trace = TRACING_BUNDLER_OVERVIEW_TARGETS.join(",");
                    }
                    "bundler" => {
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
                    .with_timer(fmt::time::SystemTime)
                    .init();

                None
            };

            let tt = TurboTasks::new(TurboTasksBackend::new(
                BackendOptions {
                    dependency_tracking: false,
                    storage_mode: None,
                    ..Default::default()
                },
                noop_backing_storage(),
            ));

            let project_options_path = PathBuf::from(&project_path).join("project_options.json");
            let mut project_options_file =
                File::open(&project_options_path).unwrap_or_else(|_| panic!("failed to load {}", project_options_path.display()));
            let config_path = PathBuf::from(&project_path).join("config.json");
            let mut config_file =
                File::open(&config_path).unwrap_or_else(|_| panic!("failed to load {}", config_path.display()));
            let mut config= String::new();
            config_file.read_to_string(&mut config).unwrap();
            
            let partial_project_options: PartialProjectOptions =
                serde_json::from_reader(&mut project_options_file).unwrap();
            let  project_options = ProjectOptions {
                root_path,
                project_path,
                entry: partial_project_options.entry.unwrap_or_default(),
                config: if config.is_empty() {
                    r#"{ "env": { },"experimental": { } }"#.into()
                    } else {
                        config.into()
                    },
                js_config: partial_project_options.js_config.unwrap_or(r#"{}"#.into()),
                env: partial_project_options.env.unwrap_or_default(),
                define_env: partial_project_options.define_env.unwrap_or_default(),
                browserslist_query: partial_project_options.browserslist_query.unwrap_or("last 1 Chrome versions, last 1 Firefox versions, last 1 Safari versions, last 1 Edge versions".into()),
                no_mangling: partial_project_options.no_mangling.unwrap_or(false),
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

   

            let result = main_inner(&tt, project_options).await;
            let memory = TurboMalloc::memory_usage();
            tracing::info!("memory usage: {} MiB", memory / 1024 / 1024);
            let start = Instant::now();
            drop(tt);
            tracing::info!("drop {:?}", start.elapsed());
            result
        })
        .unwrap();
}
