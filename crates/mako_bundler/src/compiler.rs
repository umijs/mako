use crate::{
    build::build::BuildParam, config::Config, context::Context, generate::generate::GenerateParam,
};

pub struct Compiler {
    pub context: Context,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        let context = Context::new(config);
        Self { context }
    }

    pub fn run(&mut self) {
        self.build(&BuildParam { files: None });
        self.generate(&GenerateParam { write: true });
        self.context.chunk_graph.print_graph();
    }

    pub fn _watch(&self) {}
}
