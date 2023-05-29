use std::{collections::HashSet, path::Path};

use crate::module::ModuleId;

pub type ChunkId = ModuleId;

pub enum ChunkType {
    #[allow(dead_code)]
    Runtime,
    Entry,
    Async,
}

pub struct Chunk {
    pub id: ChunkId,
    pub chunk_type: ChunkType,
    modules: HashSet<ModuleId>,
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: HashSet::from([id.clone()]),
            id,
            chunk_type,
        }
    }

    pub fn filename(&self) -> String {
        match self.chunk_type {
            ChunkType::Runtime => "runtime.js".into(),
            ChunkType::Entry => {
                // foo/bar.js -> bar.js
                let id = self.id.id.clone();
                let basename = Path::new(&id)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                format!("{}.js", basename)
            }
            ChunkType::Async => {
                let path = Path::new(&self.id.id);
                let filename = path.file_name().unwrap().to_string_lossy();
                format!("{}-async.js", &filename)
            }
        }
    }

    pub fn add_module(&mut self, module_id: ModuleId) {
        self.modules.insert(module_id);
    }

    pub fn get_modules(&self) -> &HashSet<ModuleId> {
        &self.modules
    }
}
