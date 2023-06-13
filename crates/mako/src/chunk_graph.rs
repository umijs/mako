use std::collections::{HashMap, HashSet};

use crate::chunk::{Chunk, ChunkId};
use crate::module::ModuleId;
use petgraph::stable_graph::{DefaultIx, NodeIndex, StableDiGraph};

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

    pub fn chunks_mut(&mut self) -> Vec<&mut Chunk> {
        self.graph.node_weights_mut().collect()
    }

    pub fn add_edge(&mut self, from: &ChunkId, to: &ChunkId) {
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph.add_edge(*from, *to, ());
    }

    pub fn chunk_names(&self) -> HashSet<String> {
        self.graph.node_weights().map(|c| c.filename()).collect()
    }
}
