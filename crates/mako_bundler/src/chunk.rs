use std::collections::HashSet;
use std::path::Path;

use crate::module::ModuleId;

pub type ChunkId = ModuleId;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ChunkType {
    // Runtime chunk 一般独立出来好点，包含模块信息，每次 sha 都会变
    Runtime,
    Entry,
    Async,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Chunk {
    pub id: ChunkId,
    modules: HashSet<ModuleId>,
    pub chunk_type: ChunkType,
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
            ChunkType::Entry => "bundle.js".into(),
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

    pub fn remove_module(&mut self, module_id: &ModuleId) {
        self.modules.retain(|id| id != module_id);
    }

    pub fn get_modules(&self) -> Vec<&ModuleId> {
        let mut modules: _ = self.modules.iter().collect::<Vec<&ModuleId>>();
        // sort by module id
        modules.sort_by_key(|m| m.id.to_string());

        modules
    }
}
