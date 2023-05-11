use crate::{
    build::build::BuildParam, config::Config, context::Context, generate::generate::GenerateParam,
    plugin::plugin_driver::PluginDriver,
};
use std::sync::Arc;

pub struct Compiler {
    pub context: Arc<Context>,
    pub plugin_driver: PluginDriver,
}

impl Compiler {
    pub fn new(config: Config, plugin_driver: PluginDriver) -> Self {
        let context = Context::new(config);
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
