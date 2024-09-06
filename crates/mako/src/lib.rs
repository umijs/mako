#![feature(box_patterns)]
#![feature(hasher_prefixfree_extras)]
#![feature(let_chains)]

pub mod ast;
mod build;
pub mod cli;
pub mod compiler;
pub mod config;
pub mod dev;
mod features;
mod generate;
mod module;
mod module_graph;
pub mod plugin;
mod plugins;
mod resolve;
pub mod stats;
pub mod utils;
mod visitors;

#[macro_export]
macro_rules! mako_profile_scope {
    ($id:expr) => {
        #[cfg(feature = "profile")]
        puffin::profile_scope!($id);
    };
    ($id:expr, $data:expr) => {
        #[cfg(feature = "profile")]
        puffin::profile_scope!($id, $data);
    };
}

#[macro_export]
macro_rules! mako_profile_function {
    () => {
        #[cfg(feature = "profile")]
        puffin::profile_function!();
    };
    ($data:expr) => {
        #[cfg(feature = "profile")]
        puffin::profile_function!($data);
    };
}

#[macro_export]
macro_rules! ternary {
    ($if_condition:expr, $if_stmt:expr, $else_stmt:expr) => {
        if $if_condition {
            $if_stmt
        } else {
            $else_stmt
        }
    };
}
