use std::hash::Hasher;
use std::path::{Component, Path};

use indexmap::IndexSet;
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
    pub modules: IndexSet<ModuleId>,
    pub content: Option<String>,
    pub source_map: Option<String>,
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: IndexSet::new(),
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
            // foo/bar.tsx -> foo_bar_tsx-async.js
            ChunkType::Async | ChunkType::Sync => {
                let path = Path::new(&self.id.id);

                let name = path
                    .components()
                    .filter(|c| !matches!(c, Component::RootDir | Component::CurDir))
                    .map(|c| match c {
                        Component::ParentDir => "@".to_string(),
                        Component::Prefix(_) => "@".to_string(),
                        Component::RootDir => "".to_string(),
                        Component::CurDir => "".to_string(),
                        Component::Normal(seg) => seg.to_string_lossy().replace('.', "_"),
                    })
                    .collect::<Vec<String>>()
                    .join("_");

                format!("{}-async.js", name)
            }
        }
    }

    pub fn add_module(&mut self, module_id: ModuleId) {
        if let (pos, false) = self.modules.insert_full(module_id.clone()) {
            // module already exists, move it to the back
            self.modules.shift_remove_index(pos);
            self.modules.insert(module_id);
        }
    }

    pub fn get_modules(&self) -> &IndexSet<ModuleId> {
        &self.modules
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) {
        self.modules.shift_remove(module_id);
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

#[cfg(test)]
mod tests {
    use crate::chunk::{Chunk, ChunkType};
    use crate::module::ModuleId;

    #[test]
    fn test_filename() {
        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Entry);
        assert_eq!(chunk.filename(), "bar.js");

        let chunk = Chunk::new(ModuleId::new("./foo/bar.tsx".into()), ChunkType::Async);
        assert_eq!(chunk.filename(), "foo_bar_tsx-async.js");

        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Runtime);
        assert_eq!(chunk.filename(), "runtime.js");
    }
}
