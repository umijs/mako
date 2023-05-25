use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use crate::{chunk_graph::ChunkGraph, config::Config, module_graph::ModuleGraph};

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub config: Config,
    pub root: PathBuf,
}

impl Context {
    pub fn emit_assets(&self, k: String, v: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(k, v);
        drop(assets_info);
    }
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
                assets_info: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn compile(&self) {
        self.build();
        self.generate();
    }
}
