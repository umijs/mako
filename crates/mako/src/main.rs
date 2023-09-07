#![feature(box_patterns)]

use std::sync::Arc;

use clap::Parser;
use tracing::debug;

use crate::compiler::Args;
use crate::config::Mode;
use crate::logger::init_logger;

mod analyze_deps;
mod analyze_statement;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod cli;
mod comments;
mod compiler;
mod config;
mod defined_ident_collector;
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
mod module_side_effects_flag;
mod optimize_chunk;
mod parse;
mod plugin;
mod plugins;
mod resolve;
mod sourcemap;
mod statement;
mod statement_graph;
mod stats;
mod targets;
#[cfg(test)]
mod test_helper;
mod transform;
mod transform_after_resolve;
mod transform_async_module;
mod transform_css_handler;
mod transform_css_url_replacer;
mod transform_dep_replacer;
mod transform_dynamic_import;
mod transform_env_replacer;
mod transform_in_generate;
mod transform_optimizer;
mod transform_provide;
mod transform_react;
mod tree_shaking;
mod tree_shaking_analyze;
mod tree_shaking_module;
mod unused_statement_cleanup;
mod unused_statement_marker;
mod unused_statement_sweep;
mod update;
mod used_ident_collector;
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
    let root = root.canonicalize().unwrap();

    // config
    let mut config = config::Config::new(&root, None, None).unwrap();

    config.mode = cli.mode;

    // dev 环境下不产生 hash, prod 环境下根据用户配置
    if config.mode == Mode::Development {
        config.hash = false;
    }

    debug!("config: {:?}", config);

    // compiler
    let compiler = compiler::Compiler::new(config, root.clone(), Args { watch: cli.watch });
    compiler.compile();

    if cli.watch {
        let d = crate::dev::DevServer::new(root.clone(), Arc::new(compiler));
        // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
        d.serve().await;
    }
}
