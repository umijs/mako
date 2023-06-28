use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

use petgraph::stable_graph::{DefaultIx, NodeIndex, StableDiGraph};
use twox_hash::XxHash64;

use crate::chunk::{Chunk, ChunkId};
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

pub struct ChunkGraph {
    graph: StableDiGraph<Chunk, ()>,
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
}

impl ChunkGraph {
    pub fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            id_index_map: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.graph.clear();
        self.id_index_map.clear();
    }

    pub fn add_chunk(&mut self, chunk: Chunk) {
        let chunk_id = chunk.id.clone();
        let node_index = self.graph.add_node(chunk);
        self.id_index_map.insert(chunk_id, node_index);
    }

    #[allow(dead_code)]
    pub fn has_chunk(&self, chunk_id: &ChunkId) -> bool {
        self.id_index_map.contains_key(chunk_id)
    }

    pub fn get_chunks(&self) -> Vec<&Chunk> {
        self.graph.node_weights().collect()
    }

    pub fn get_chunk_by_name(&self, name: &String) -> Option<&Chunk> {
        self.graph.node_weights().find(|c| c.filename().eq(name))
    }

    pub fn add_edge(&mut self, from: &ChunkId, to: &ChunkId) {
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph.add_edge(*from, *to, ());
    }

    pub fn chunk_names(&self) -> HashSet<String> {
        self.graph.node_weights().map(|c| c.filename()).collect()
    }

    pub fn full_hash(&self, module_graph: &ModuleGraph) -> u64 {
        let mut chunks = self.get_chunks();
        chunks.sort_by_key(|c| c.id.id.clone());

        let mut hasher: XxHash64 = Default::default();
        for c in chunks {
            hasher.write_u64(c.hash(module_graph))
        }
        hasher.finish()
    }
}

impl Default for ChunkGraph {
    fn default() -> Self {
        Self::new()
    }
}
