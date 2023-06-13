use std::collections::HashMap;

use petgraph::stable_graph::{DefaultIx, NodeIndex, StableDiGraph};

use crate::chunk::{Chunk, ChunkId};
use crate::module::ModuleId;

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

    pub fn add_edge(&mut self, from: &ChunkId, to: &ChunkId) {
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph.add_edge(*from, *to, ());
    }
}
