use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

use twox_hash::XxHash64;

use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

pub type ChunkId = ModuleId;

pub enum ChunkType {
    #[allow(dead_code)]
    Runtime,
    Entry,
    Async,
    // mean that the chunk is not async, but it's a dependency of an async chunk
    Sync,
}

pub struct Chunk {
    pub id: ChunkId,
    pub chunk_type: ChunkType,
    modules: HashSet<ModuleId>,
    pub content: Option<String>,
    pub source_map: Option<String>,
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: HashSet::new(),
            id,
            chunk_type,
            content: None,
            source_map: None,
        }
    }

    pub fn new_for(id: ModuleId, chunk_type: ChunkType) -> Self {
        let mut c = Chunk::new(ChunkId::new(id.id.clone()), chunk_type);
        c.add_module(id);
        c
    }

    pub fn filename(&self) -> String {
        match self.chunk_type {
            ChunkType::Runtime => "runtime.js".into(),
            // foo/bar.tsx -> bar.js
            ChunkType::Entry => {
                let id = self.id.id.clone();
                let basename = Path::new(&id)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                format!("{}.js", basename)
            }
            // foo/bar.tsx -> bar-{hash}-async.js
            ChunkType::Async | ChunkType::Sync => {
                let path = Path::new(&self.id.id);
                let hash = hash_path(path);
                let filename = path.file_stem().unwrap().to_string_lossy();
                format!("{}-{}-async.js", &filename, hash)
            }
        }
    }

    pub fn add_module(&mut self, module_id: ModuleId) {
        self.modules.insert(module_id);
    }

    pub fn get_modules(&self) -> &HashSet<ModuleId> {
        &self.modules
    }

    pub fn mut_modules(&mut self) -> &mut HashSet<ModuleId> {
        &mut self.modules
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) {
        self.modules.remove(module_id);
    }

    pub fn has_module(&self, module_id: &ModuleId) -> bool {
        self.modules.contains(module_id)
    }

    pub fn hash(&self, mg: &ModuleGraph) -> u64 {
        let mut sorted_module_ids = self.modules.iter().cloned().collect::<Vec<ModuleId>>();
        sorted_module_ids.sort_by_key(|m| m.id.clone());

        let mut hash: XxHash64 = Default::default();

        for id in sorted_module_ids {
            let m = mg.get_module(&id).unwrap();

            hash.write_u64(m.info.as_ref().unwrap().raw_hash);
        }

        hash.finish()
    }
}

fn hash_path<P: AsRef<std::path::Path>>(path: P) -> u64 {
    let path_str = path.as_ref().to_str().expect("Path is not valid UTF-8");
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use crate::chunk::{Chunk, ChunkType};
    use crate::module::ModuleId;

    #[test]
    fn test_filename() {
        let chunk = Chunk::new_for(ModuleId::new("foo/bar.tsx".into()), ChunkType::Entry);
        assert_eq!(chunk.filename(), "bar.js");

        let chunk = Chunk::new_for(ModuleId::new("foo/bar.tsx".into()), ChunkType::Async);
        assert_eq!(chunk.filename(), "bar-15149280808876942159-async.js");

        let chunk = Chunk::new_for(ModuleId::new("foo/bar.tsx".into()), ChunkType::Runtime);
        assert_eq!(chunk.filename(), "runtime.js");
    }
}
