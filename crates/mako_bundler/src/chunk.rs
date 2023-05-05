use std::collections::HashSet;

use crate::module::ModuleId;

pub type ChunkId = ModuleId;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ChunkType {
    Entry,
    Async,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Chunk {
    pub id: ChunkId,
    modules: HashSet<ModuleId>,
    chunk_type: ChunkType,
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: HashSet::from([id.clone()]),
            id,
            chunk_type,
        }
    }

    pub fn add_module(&mut self, module_id: ModuleId) {
        self.modules.insert(module_id);
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) {
        self.modules.retain(|id| id != module_id);
    }

    pub fn modules(&self) -> &HashSet<ModuleId> {
        &self.modules
    }
}
