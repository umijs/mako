#![feature(box_patterns)]

use std::sync::Arc;

use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

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
mod dev;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
mod load;
mod minify;
mod module;
mod module_graph;
mod parse;
mod resolve;
mod sourcemap;
#[cfg(test)]
mod test_helper;
mod transform;
mod transform_css_handler;
mod transform_dep_replacer;
mod transform_dynamic_import;
mod transform_env_replacer;
mod transform_in_generate;
mod transform_optimizer;
mod update;
mod watch;

#[tokio::main]
async fn main() {
    // logger
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mako=info")),
        )
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .without_time()
        .init();

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
    let mut config = config::Config::new(&root).unwrap();
    config.mode = cli.mode;
    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone());
    compiler.compile();

    if cli.watch {
        let d = crate::dev::DevServer::new(root.clone(), Arc::new(compiler));

        d.serve().await;
    }
}
