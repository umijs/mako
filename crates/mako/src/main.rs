#![feature(box_patterns)]
#![feature(let_chains)]
#![feature(result_option_inspect)]

use std::fs;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::clap::Parser;
#[cfg(feature = "profile")]
use mako_core::tokio::sync::Notify;
use mako_core::tracing::debug;

use crate::compiler::Args;
use crate::logger::init_logger;
#[cfg(feature = "profile")]
use crate::profile_gui::ProfileApp;

mod analyze_deps;
mod ast;
mod ast_2;
mod build;
mod chunk;
mod chunk_graph;
mod chunk_pot;
mod cli;
mod comments;
mod compiler;
mod config;
mod dev;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
mod load;
mod logger;
mod minify;
mod module;
mod module_graph;
mod optimize_chunk;
mod parse;
mod plugin;
mod plugins;
#[cfg(feature = "profile")]
mod profile_gui;
mod resolve;
mod runtime;
mod sourcemap;
mod stats;
mod swc_helpers;
mod targets;
#[cfg(test)]
mod test_helper;
mod thread_pool;
// mod tokio_runtime;
mod transform;
mod transform_in_generate;
mod transformers;
mod tree_shaking;
mod update;
mod util;
mod visitors;
#[cfg(not(target_family = "wasm"))]
mod watch;

#[cfg(all(not(target_os = "linux"), not(target_family = "wasm")))]
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

    // let root = root
    //     .canonicalize()
    //     .map_err(|e| anyhow!("Root directory {:?} not found {}", root, e))?;
    //
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

    compiler.compile()
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

    for entry in fs::read_dir("/examples/normal")? {
        let dir = entry?;
        println!("{:?}", dir.path());
    }

    let root = if cli.root.is_absolute() {
        cli.root
    } else {
        std::env::current_dir()?.join(cli.root)
    };

    // let root = root
    //     .canonicalize()
    //     .map_err(|e| anyhow!("Root directory {:?} not found {}", root, e))?;
    //
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
            let d = crate::dev::DevServer::new(root.clone(), compiler);
            // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
            d.serve(move |_params| {}).await;
        }
    }
    Ok(())
}
