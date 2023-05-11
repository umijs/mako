#![feature(box_patterns)]

use compiler::Compiler;
use std::sync::{Arc, Mutex};

use crate::{plugin::plugin_driver::PluginDriver, plugins::node_polyfill::NodePolyfillPlugin};

pub mod build;
pub mod chunk;
pub mod chunk_graph;
pub mod compiler;
pub mod config;
pub mod context;
pub mod generate;
pub mod module;
pub mod module_graph;
pub mod plugin;
pub mod plugins;
pub mod utils;

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        panic!("Please specify the root directory of the project");
    }

    // plugin driver
    let mut plugin_driver = PluginDriver::new();

    // register plugins
    plugin_driver.register(NodePolyfillPlugin {});

    // config
    let root = std::env::current_dir()
        .unwrap()
        .join(&args[1])
        .to_string_lossy()
        .to_string();
    let mut config = config::Config::from_literal_str(
        format!(
            r#"
{{
    "externals": {{}},
    "root": "{}",
    "entry": {{ "index": "index.tsx" }}
}}
    "#,
            root
        )
        .as_str(),
    )
    .unwrap();

    // allow plugin to modify config
    let config_lock = Arc::new(Mutex::new(&mut config));

    plugin_driver
        .run_hook_serial(|p, _last_ret| {
            p.config(&mut config_lock.lock().unwrap())?;
            Ok(Some(()))
        })
        .unwrap();

    config.normalize();

    // compiler_origin::run_compiler(config);
    let mut compiler = Compiler::new(config, plugin_driver);
    compiler.run();

    println!("✅ DONE");
}
