#![feature(box_patterns)]
#![feature(hasher_prefixfree_extras)]
#![feature(let_chains)]
#![feature(result_option_inspect)]

pub mod ast;
mod build;
pub mod cli;
pub mod compiler;
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod dev;
mod features;
mod generate;
mod module;
mod module_graph;
pub mod plugin;
mod plugins;
mod resolve;
mod stats;
pub mod utils;
mod visitors;
