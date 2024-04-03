#![feature(box_patterns)]
#![feature(let_chains)]
#![feature(result_option_inspect)]

use std::sync::Arc;

use mako::compiler::{self, Args};
use mako::logger::init_logger;
#[cfg(feature = "profile")]
use mako::profile_gui::ProfileApp;
use mako::{cli, config, dev, tokio_runtime};
use mako_core::anyhow::{anyhow, Result};
use mako_core::clap::Parser;
#[cfg(feature = "profile")]
use mako_core::tokio::sync::Notify;
use mako_core::tracing::debug;

#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: mimalloc_rust::GlobalMiMalloc = mimalloc_rust::GlobalMiMalloc;

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() -> Result<()> {
    let fut = async { run().await };

    tokio_runtime::block_on(fut)
}

async fn run() -> Result<()> {
    // logger
    init_logger();

    // cli
    let cli = cli::Cli::parse();
    debug!(
        "cli: watch = {}, mode = {}, root = {}",
        cli.watch,
        cli.mode,
        cli.root.to_str().unwrap()
    );
    let root = if cli.root.is_absolute() {
        cli.root
    } else {
        std::env::current_dir()?.join(cli.root)
    };
    let root = root
        .canonicalize()
        .map_err(|_| anyhow!("Root directory {:?} not found", root))?;

    // config
    let cli_args = format!(
        r#"
        {{
            "mode": "{}"
        }}
        "#,
        cli.mode
    );
    let mut config = config::Config::new(&root, None, Some(cli_args.as_str()))
        .map_err(|e| anyhow!(format!("Load config failed: {}", e)))?;

    config.mode = cli.mode;

    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone(), Args { watch: cli.watch }, None)?;
    let compiler = Arc::new(compiler);

    #[cfg(feature = "profile")]
    {
        let notify = Arc::new(Notify::new());
        let to_be_notify = notify.clone();

        tokio_runtime::spawn(async move {
            let compiler = compiler.clone();

            to_be_notify.notified().await;

            compiler.compile().unwrap();

            if cli.watch {
                let d = crate::dev::DevServer::new(root.clone(), compiler.clone());
                d.serve(move |_params| {}).await;
            }
        });

        mako_core::puffin::set_scopes_on(true);
        let native_options = Default::default();
        let _ = mako_core::eframe::run_native(
            "puffin egui eframe",
            native_options,
            Box::new(move |_cc| Box::new(ProfileApp::new(notify))),
        );
    }

    #[cfg(not(feature = "profile"))]
    {
        if let Err(e) = compiler.compile() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        if cli.watch {
            let d = dev::DevServer::new(root.clone(), compiler);
            // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
            d.serve(move |_params| {}).await;
        }
    }
    Ok(())
}
