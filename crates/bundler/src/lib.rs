#![feature(future_join)]
#![feature(min_specialization)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

/* Should be deleted */
pub mod arguments;
mod bundle;
mod contexts;
mod dev_runtime;
mod env;
mod issue;
mod util;
/* Should be deleted */

/* A full design benchmarked to next-api */
pub mod config;
pub mod emit;
pub mod endpoint;
pub mod paths;
pub mod project;
pub mod runtime;
pub mod transforms;
pub mod versioned_content_map;
/* A full design benchmarked to next-api */

/* Should be deleted */
pub use bundle::build::build;
pub use bundle::dev::dev;
/* Should be deleted */

pub fn register() {
    turbopack::register();
    turbopack_nodejs::register();
    turbopack_browser::register();
    turbopack_ecmascript_plugins::register();
    turbopack_resolve::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}
