use std::hash::Hasher;
use std::path::{Component, Path};

use indexmap::IndexSet;
use twox_hash::XxHash64;

use crate::module::{ModuleId, ModuleType};
use crate::module_graph::ModuleGraph;

pub type ChunkId = ModuleId;

#[derive(PartialEq, Eq)]
pub enum ChunkType {
    #[allow(dead_code)]
    Runtime,
    // module id, entry chunk name
    Entry(ModuleId, String),
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

    pub fn js_hash(&self, mg: &ModuleGraph) -> u64 {
        get_related_module_hash(self, mg, false)
    }
    pub fn css_hash(&self, mg: &ModuleGraph) -> u64 {
        get_related_module_hash(self, mg, true)
    }

    pub fn js_chunk_name_with_hash(&self, mg: &ModuleGraph) -> String {
        let filename = self.filename();
        let hash = self.js_hash(mg);
        self.with_hash(&filename, hash, "js")
    }

    pub fn css_chunk_name_with_hash(&self, mg: &ModuleGraph) -> String {
        let filename = self.filename();
        let hash = self.css_hash(mg);
        self.with_hash(&filename, hash, "css")
    }

    fn with_hash(&self, filename: &String, hash: u64, ext: &str) -> String {
        let filename = if let Some((left, _)) = filename.rsplit_once('.') {
            left
        } else {
            filename
        };

        let hash = &format!("{:08x}", hash)[0..8];

        format!("{}.{}.{}", filename, hash, ext)
    }

    pub fn filename(&self) -> String {
        match &self.chunk_type {
            ChunkType::Runtime => "runtime.js".into(),
            // foo/bar.tsx -> bar.js
            ChunkType::Entry(_, name) => format!("{}.js", name),
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

    #[allow(dead_code)]
    pub fn mut_modules(&mut self) -> &mut IndexSet<ModuleId> {
        &mut self.modules
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

// 给 output_ast 计算 hash 值，get_chunk_emit_files 时会根据此 hash 值做缓存
fn get_related_module_hash(
    chunk: &crate::chunk::Chunk,
    module_graph: &crate::module_graph::ModuleGraph,
    is_css_ast: bool,
) -> u64 {
    let mut hash: XxHash64 = Default::default();
    let mut module_ids_used = chunk
        .get_modules()
        .iter()
        .cloned()
        .collect::<Vec<ModuleId>>();
    // 因为存在 code splitting，可能存在用户引入依赖的顺序发生改变但依赖背后的 module 没有改变的情况
    // 此时 js chunk 不需要重新生成，所以在计算 ast_module_hash 针对 js 的场景先对 module 做轮排序
    if !is_css_ast {
        module_ids_used.sort_by_key(|m| m.id.clone());
    }

    for id in module_ids_used {
        let m = module_graph.get_module(&id).unwrap();
        let m_type = m.get_module_type();

        if matches!(m_type, ModuleType::Css) == is_css_ast {
            hash.write_u64(m.info.as_ref().unwrap().raw_hash);
        }
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use crate::chunk::{Chunk, ChunkType};
    use crate::module::ModuleId;

    #[test]
    fn test_filename() {
        let module_id = ModuleId::new("foo/bar.tsx".into());
        let chunk = Chunk::new(
            module_id.clone(),
            ChunkType::Entry(module_id, "foo_bar".to_string()),
        );
        assert_eq!(chunk.filename(), "foo_bar.js");

        let chunk = Chunk::new(ModuleId::new("./foo/bar.tsx".into()), ChunkType::Async);
        assert_eq!(chunk.filename(), "foo_bar_tsx-async.js");

        let chunk = Chunk::new(ModuleId::new("foo/bar.tsx".into()), ChunkType::Runtime);
        assert_eq!(chunk.filename(), "runtime.js");
    }
}
