#![feature(box_patterns)]

use std::sync::Arc;

use clap::Parser;
use tracing::debug;

use crate::logger::init_logger;

mod analyze_deps;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod cli;
mod compiler;
mod config;
mod config_node_polyfill;
mod copy;
mod css_modules;
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
mod parse;
mod resolve;
mod sourcemap;
mod targets;
#[cfg(test)]
mod test_helper;
mod transform;
mod transform_css_handler;
mod transform_dep_replacer;
mod transform_dynamic_import;
mod transform_env_replacer;
mod transform_in_generate;
mod transform_optimizer;
mod transform_provide;
mod update;
mod watch;

#[tokio::main]
async fn main() {
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
        std::env::current_dir().unwrap().join(cli.root)
    };

    // config
    let mut config = config::Config::new(&root, None, None).unwrap();
    config.mode = cli.mode;
    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone());
    compiler.compile();

    if cli.watch {
        let d = crate::dev::DevServer::new(root.clone(), Arc::new(compiler));
        //TODO when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
        d.serve().await;
    }
}
