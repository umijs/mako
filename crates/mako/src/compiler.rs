use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use crate::{chunk_graph::ChunkGraph, config::Config, module_graph::ModuleGraph};

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub config: Config,
    pub root: PathBuf,
}

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(config: Config, root: PathBuf) -> Self {
        assert!(root.is_absolute(), "root path must be absolute");
        Self {
            context: Arc::new(Context {
                config,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
            }),
        }
    }

    pub fn compile(&self) {
        self.build();
        self.generate();
    }
}
