#![feature(box_patterns)]
#![feature(hasher_prefixfree_extras)]
#![feature(is_some_with)]
#![feature(box_syntax)]

mod analyze_deps;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod chunk_pot;
mod cli;
mod comments;
pub mod compiler;
pub mod config;
pub mod dev;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
pub mod load;
pub mod logger;
mod minify;
mod module;
mod module_graph;
mod optimize_chunk;
mod parse;
pub mod plugin;
mod plugins;
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
mod util;
mod watch;
