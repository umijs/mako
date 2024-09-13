use std::fmt::{Debug, Formatter};
use std::hash::Hasher;
use std::path::{Component, Path};

use hashlink::LinkedHashSet;
use twox_hash::XxHash64;

use crate::ast::file::parse_path;
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;
use crate::utils::url_safe_base64_encode;

pub type ChunkId = ModuleId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ChunkType {
    #[allow(dead_code)]
    Runtime,
    /**
     * Entry(chunk_id, chunk_name, is_shared_chunk)
     */
    Entry(ModuleId, String, bool),
    Async,
    // mean that the chunk is not async, but it's a dependency of an async chunk
    Sync,
    // web workers
    Worker(ModuleId),
}

pub struct Chunk {
    pub id: ChunkId,
    pub chunk_type: ChunkType,
    pub modules: LinkedHashSet<ModuleId>,
    pub content: Option<String>,
    pub source_map: Option<String>,
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}#{}({:?})",
            self.id.id,
            self.modules.len(),
            self.chunk_type
        )?;
        Ok(())
    }
}

impl Chunk {
    pub fn new(id: ChunkId, chunk_type: ChunkType) -> Self {
        Self {
            modules: LinkedHashSet::new(),
            id,
            chunk_type,
            content: None,
            source_map: None,
        }
    }

    pub fn filename(&self) -> String {
        match &self.chunk_type {
            ChunkType::Runtime => "runtime.js".into(),
            // foo/bar.tsx -> bar.js
            ChunkType::Entry(_, name, _) => format!("{}.js", name),
            // foo/bar.tsx -> foo_bar_tsx-async.js
            ChunkType::Async | ChunkType::Sync | ChunkType::Worker(_) => {
                let (path, search, ..) = parse_path(&self.id.id).unwrap();
                let path = Path::new(&path);

                let mut name = path
                    .components()
                    .filter(|c| !matches!(c, Component::RootDir | Component::CurDir))
                    .map(|c| match c {
                        Component::ParentDir => "pd_".to_string(),
                        Component::Prefix(_) => "ps_".to_string(),
                        Component::RootDir => "".to_string(),
                        Component::CurDir => "".to_string(),
                        Component::Normal(seg) => {
                            seg.to_string_lossy().replace(['.', '?', '@'], "_")
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("_");

                if !search.is_empty() {
                    let search_hash =
                        url_safe_base64_encode(md5::compute(search).0)[..4].to_string();
                    name = format!("{}_q_{}", name, search_hash);
                }

                format!(
                    "{}-{}.js",
                    name,
                    if matches!(self.chunk_type, ChunkType::Worker(_)) {
                        "worker"
                    } else {
                        "async"
                    }
                )
            }
        }
    }

    pub fn add_module(&mut self, module_id: ModuleId) {
        self.modules.insert(module_id);
    }

    pub fn get_modules(&self) -> &LinkedHashSet<ModuleId> {
        &self.modules
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) {
        self.modules.remove(module_id);
    }

    pub fn has_module(&self, module_id: &ModuleId) -> bool {
        self.modules.contains(module_id)
    }

    pub fn hash(&self, mg: &ModuleGraph) -> u64 {
        let mut sorted_module_ids = self.modules.iter().cloned().collect::<Vec<ModuleId>>();
        sorted_module_ids.sort_by_key(|m| m.id.to_string());

        let mut hash: XxHash64 = Default::default();

        for id in sorted_module_ids {
            let m = mg.get_module(&id).unwrap();

            if let Some(info) = &m.info {
                hash.write_u64(info.raw_hash);
            } else {
                hash.write(m.id.id.as_bytes());
            }
        }

        hash.finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::chunk::{Chunk, ChunkType};
    use crate::module::ModuleId;

    #[test]
    fn test_filename() {
        let module_id = ModuleId::new("foo/bar.tsx".into());
        let chunk = Chunk::new(
            module_id.clone(),
            ChunkType::Entry(module_id, "foo_bar".to_string(), false),
        );
        assert_eq!(chunk.filename(), "foo_bar.js");

        let chunk = Chunk::new(ModuleId::new("./foo/bar.tsx".into()), ChunkType::Async);
        assert_eq!(chunk.filename(), "foo_bar_tsx-async.js");

        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Runtime);
        assert_eq!(chunk.filename(), "runtime.js");
    }
}
