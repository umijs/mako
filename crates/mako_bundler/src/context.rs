use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use maplit::hashset;
use nodejs_resolver::{Options, Resolver};

use crate::{chunk_graph::ChunkGraph, config::Config, module_graph::ModuleGraph};

pub struct Context {
    pub config: Config,
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub resolver: Arc<Resolver>,
}

impl Context {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            resolver: Arc::new(Resolver::new(Options {
                extensions: vec![
                    ".js".to_string(),
                    ".jsx".to_string(),
                    ".ts".to_string(),
                    ".tsx".to_string(),
                    ".mjs".to_string(),
                    ".cjs".to_string(),
                ],
                condition_names: hashset! {
                    "node".to_string(),
                    "require".to_string(),
                    "import".to_string(),
                    "browser".to_string(),
                    "default".to_string()
                },
                external_cache: Some(Arc::new(Default::default())),
                ..Default::default()
            })),
        }
    }

    pub fn emit_assets(&self, k: String, v: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(k, v);
        drop(assets_info);
    }
}
