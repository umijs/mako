#![feature(box_patterns)]
#![feature(is_some_with)]
#![feature(box_syntax)]

use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::clap::Parser;
use mako_core::tokio;
#[cfg(feature = "profile")]
use mako_core::tokio::sync::Notify;
use mako_core::tracing::debug;

use crate::compiler::Args;
use crate::config::Mode;
use crate::logger::init_logger;
#[cfg(feature = "profile")]
use crate::profile_gui::ProfileApp;

mod analyze_deps;
mod ast;
mod bfs;
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
mod sourcemap;
mod stats;
mod targets;
#[cfg(test)]
mod test_helper;
mod transform;
mod transform_in_generate;
mod transformers;
mod tree_shaking;
mod update;
mod watch;

#[tokio::main]
async fn main() -> Result<()> {
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
        .map_err(|_| anyhow!("The root directory {:?} is not found", root))?;

    // config
    let mut config =
        config::Config::new(&root, None, None).map_err(|_| anyhow!("load config error"))?;

    config.mode = cli.mode;

    // dev 环境下不产生 hash, prod 环境下根据用户配置
    if config.mode == Mode::Development {
        config.hash = false;
    }

    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone(), Args { watch: cli.watch })?;
    let compiler = Arc::new(compiler);

    #[cfg(feature = "profile")]
    {
        let notify = Arc::new(Notify::new());
        let to_be_notify = notify.clone();

        tokio::spawn(async move {
            let compiler = compiler.clone();

            to_be_notify.notified().await;

            compiler.compile().unwrap();

            if cli.watch {
                let d = crate::dev::DevServer::new(root.clone(), compiler.clone());
                d.serve().await;
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
        compiler.compile()?;
        if cli.watch {
            let d = crate::dev::DevServer::new(root.clone(), compiler);
            // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
            d.serve().await;
        }
    }
    Ok(())
}
