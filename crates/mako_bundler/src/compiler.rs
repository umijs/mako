use crate::{config::Config, context::Context};

pub struct Compiler {
    pub context: Context,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        let context = Context::new(config);
        Self { context }
    }

    pub fn run(&mut self) {
        self.build();
        self.generate();
    }

    pub fn _watch(&self) {}
}
