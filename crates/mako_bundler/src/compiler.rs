use crate::{
    build::build::BuildParam, config::Config, context::Context, generate::generate::GenerateParam,
};
use std::sync::Arc;

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        let context = Context::new(config);
        Self {
            context: Arc::new(context),
        }
    }

    pub fn run(&mut self) {
        self.build(&BuildParam { files: None });
        self.generate(&GenerateParam { write: true });
    }

    pub fn _watch(&self) {}

    pub fn update() {}
}
