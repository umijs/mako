#![feature(box_patterns)]

pub mod compiler;
pub mod config;

mod analyze_deps;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod cli;
mod config_node_polyfill;
mod copy;
mod generate;
mod generate_chunks;
mod group_chunk;
mod load;
mod minify;
mod module;
mod module_graph;
mod parse;
mod resolve;
mod sourcemap;
mod transform;
mod transform_css_handler;
mod transform_dep_replacer;
mod transform_dynamic_import;
mod transform_env_replacer;
mod transform_in_generate;
mod watch;
