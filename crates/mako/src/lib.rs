#![feature(box_patterns)]
#![feature(hasher_prefixfree_extras)]
#![feature(let_chains)]
#![feature(result_option_inspect)]

mod analyze_deps;
mod ast;
pub mod ast_2;
mod build;
mod chunk;
mod chunk_graph;
mod chunk_pot;
pub mod cli;
mod comments;
pub mod compiler;
pub mod config;
pub mod dev;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
mod load;
pub mod logger;
mod minify;
mod module;
mod module_graph;
mod optimize_chunk;
mod parse;
pub mod plugin;
mod plugins;
pub mod profile_gui;
mod resolve;
mod runtime;
mod sourcemap;
mod stats;
mod swc_helpers;
mod targets;
#[cfg(test)]
mod test_helper;
mod thread_pool;
pub mod tokio_runtime;
mod transform;
mod transform_in_generate;
mod update;
mod util;
mod visitors;
mod watch;
