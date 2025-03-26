#![feature(arbitrary_self_types_pointers)]

pub mod endpoints;
pub mod entrypoints;
pub mod library;
pub mod project;
pub mod versioned_content_map;
pub mod webpack_stats;

pub fn register() {
    bundler_core::register();
    turbopack_nodejs::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}
