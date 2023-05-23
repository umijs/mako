#![feature(box_patterns)]

use crate::watch::start_watch;
use crate::{
    plugin::plugin_driver::PluginDriver,
    plugins::{copy::CopyPlugin, node_polyfill::NodePolyfillPlugin},
};
use compiler::Compiler;
use std::fmt::Error;
use std::sync::{Arc, Mutex};

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
pub mod test_helper;
pub mod utils;
pub(crate) mod watch;

pub struct Bundler {
    compiler: Compiler,
}

impl Bundler {
    pub fn new(mut config: config::Config) -> Self {
        // plugin driver
        let mut plugin_driver = PluginDriver::new();

        // register plugins
        plugin_driver.register(NodePolyfillPlugin {});
        plugin_driver.register(CopyPlugin {});

        config.normalize();
        // allow plugin to modify config

        let config_lock = Arc::new(Mutex::new(&mut config));

        plugin_driver
            .run_hook_serial(|p, _last_ret| {
                p.config(&mut config_lock.lock().unwrap())?;
                Ok(Some(()))
            })
            .unwrap();

        let compiler = Compiler::new(config, plugin_driver);

        Self { compiler }
    }

    pub fn run(&mut self, watch: bool) -> Result<(), Error> {
        self.compiler.run()?;
        println!("✅Done");

        if watch {
            let root = self.compiler.context.config.root.clone();
            start_watch(&root, &mut self.compiler);
        }

        Ok(())
    }
}
