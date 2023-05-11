use std::sync::{Arc, Mutex};

use crate::plugins::node_polyfill::NodePolyfillPlugin;
use crate::{
    build::build::BuildParam, config::Config, context::Context, generate::generate::GenerateParam,
    plugin::plugin_driver::PluginDriver,
};

pub struct Compiler {
    pub context: Arc<Context>,
    pub plugin_driver: PluginDriver,
}

impl Compiler {
    pub fn new(config: &mut Config) -> Self {
        // plugin driver
        let config_lock = Arc::new(Mutex::new(config));

        let mut plugin_driver = PluginDriver::new();
        // register plugins
        plugin_driver.register(NodePolyfillPlugin {});
        plugin_driver
            .run_hook_serial(|p, _last_ret| {
                p.config(&mut config_lock.lock().unwrap())?;
                Ok(Some(()))
            })
            .unwrap();

        let context = Context::new(config_lock.lock().unwrap().clone());

        Self {
            context: Arc::new(context),
            plugin_driver,
        }
    }

    pub fn run(&mut self) {
        self.build(&BuildParam { files: None });
        self.generate(&GenerateParam { write: true });
    }

    pub fn _watch(&self) {}

    pub fn update() {}
}
