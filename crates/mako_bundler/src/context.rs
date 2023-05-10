use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use crate::{
    chunk_graph::ChunkGraph, config::Config, module_graph::ModuleGraph,
    plugin::plugin_driver::PluginDriver,
};

pub struct Context {
    pub config: Config,
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub plugin_driver: PluginDriver,
}

impl Context {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            plugin_driver: PluginDriver::new(),
        }
    }

    pub fn emit_assets(&self, k: String, v: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(k, v);
        drop(assets_info);
    }
}
