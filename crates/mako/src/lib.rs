#![feature(box_patterns)]

mod analyze_deps;
mod ast;
mod bfs;
mod build;
mod chunk;
mod chunk_graph;
mod cli;
pub mod compiler;
pub mod config;
mod config_node_polyfill;
mod copy;
mod css_modules;
mod generate;
mod generate_chunks;
mod group_chunk;
mod hmr;
mod load;
pub mod logger;
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
