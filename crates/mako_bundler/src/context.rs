use crate::{config::Config, module_graph::ModuleGraph};

pub struct Context {
    pub config: Config,
    pub module_graph: ModuleGraph,
}

impl Context {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            module_graph: ModuleGraph::new(),
        }
    }
}
