use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

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
    pub content: Option<String>,
    pub source_map: Option<String>,
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: HashSet::from([id.clone()]),
            id,
            chunk_type,
            content: None,
            source_map: None,
        }
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
            ChunkType::Async => {
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

    pub fn has_module(&self, module_id: &ModuleId) -> bool {
        self.modules.contains(module_id)
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
        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Entry);
        assert_eq!(chunk.filename(), "bar.js");

        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Async);
        assert_eq!(chunk.filename(), "bar-15149280808876942159-async.js");

        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Runtime);
        assert_eq!(chunk.filename(), "runtime.js");
    }
}
