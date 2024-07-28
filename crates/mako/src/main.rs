#![feature(box_patterns)]
#![feature(let_chains)]
#![feature(result_option_inspect)]

use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use mako::cli::Cli;
use mako::compiler::{self, Args};
use mako::config;
#[cfg(not(feature = "profile"))]
use mako::dev;
use mako::utils::logger::init_logger;
#[cfg(feature = "profile")]
use mako::utils::profile_gui::ProfileApp;
use mako::utils::tokio_runtime;
use tracing::debug;

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
    let Cli {
        watch, root, mode, ..
    } = Cli::parse();
    debug!(
        "cli: watch = {}, mode = {}, root = {}",
        watch,
        mode,
        root.to_str().unwrap()
    );
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()?.join(root)
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
        mode
    );
    let mut config = config::Config::new(&root, None, Some(cli_args.as_str()))
        .map_err(|e| anyhow!(format!("Load config failed: {}", e)))?;

    config.mode = mode;

    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone(), Args { watch }, None)?;
    let compiler = Arc::new(compiler);

    #[cfg(feature = "profile")]
    {
        puffin::set_scopes_on(true);
        let native_options = Default::default();
        let for_profile = compiler.clone();
        let _ = eframe::run_native(
            "puffin egui eframe",
            native_options,
            Box::new(move |_cc| Box::new(ProfileApp::new(for_profile))),
        );
    }

    #[cfg(not(feature = "profile"))]
    {
        if let Err(e) = compiler.compile() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        if watch {
            let d = dev::DevServer::new(root.clone(), compiler);
            // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
            d.serve(move |_params| {}).await;
        }
    }
    Ok(())
}
