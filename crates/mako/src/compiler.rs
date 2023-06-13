use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use swc_common::sync::Lrc;
use swc_common::SourceMap;

use crate::chunk_graph::ChunkGraph;
use crate::config::Config;
use crate::module_graph::ModuleGraph;

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub config: Config,
    pub root: PathBuf,
    pub meta: Meta,
}

pub struct Meta {
    pub script: ScriptMeta,
    pub css: CssMeta,
}

impl Meta {
    pub fn new() -> Self {
        Self {
            script: ScriptMeta::new(),
            css: CssMeta::new(),
        }
    }
}

pub struct ScriptMeta {
    pub cm: Lrc<SourceMap>,
}

impl ScriptMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
        }
    }
}

pub struct CssMeta {
    pub cm: Lrc<SourceMap>,
}

impl CssMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
        }
    }
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
                meta: Meta::new(),
            }),
        }
    }

    pub fn compile(&self) {
        self.build();
        self.generate();
    }
}
