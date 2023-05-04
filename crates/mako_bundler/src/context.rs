use std::collections::HashMap;

use crate::{chunk_graph::ChunkGraph, config::Config, module_graph::ModuleGraph};

pub struct Context {
    pub config: Config,
    pub module_graph: ModuleGraph,
    pub chunk_graph: ChunkGraph,
    pub assets_info: HashMap<String, String>,
}

impl Context {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            module_graph: ModuleGraph::new(),
            chunk_graph: ChunkGraph::new(),
            assets_info: HashMap::new(),
        }
    }

    pub fn emit_assets(&mut self, k: String, v: String) {
        self.assets_info.insert(k, v);
    }
}
