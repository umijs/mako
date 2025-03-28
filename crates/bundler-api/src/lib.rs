#![feature(str_split_remainder)]
#![feature(impl_trait_in_assoc_type)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]
#![feature(iter_intersperse)]

pub mod endpoints;
pub mod entrypoints;
pub mod library;
pub mod ntf_json;
pub mod operation;
pub mod paths;
pub mod project;
pub mod versioned_content_map;
pub mod webpack_stats;

pub fn register() {
    bundler_core::register();
    turbopack_nodejs::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}
