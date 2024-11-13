use core::fmt;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

use petgraph::stable_graph::{DefaultIx, NodeIndex, StableDiGraph};
use petgraph::visit::Dfs;
use petgraph::Direction;
use twox_hash::XxHash64;

use crate::generate::chunk::{Chunk, ChunkId, ChunkType};
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

pub struct ChunkGraph {
    pub(crate) graph: StableDiGraph<Chunk, ()>,
    id_index_map: HashMap<ChunkId, NodeIndex<DefaultIx>>,
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
        self.get_all_chunks()
            .into_iter()
            .filter(|c| !c.modules.is_empty())
            .collect()
    }

    pub fn get_all_chunks(&self) -> Vec<&Chunk> {
        self.graph.node_weights().collect()
    }

    pub fn mut_chunks(&mut self) -> Vec<&mut Chunk> {
        self.graph.node_weights_mut().collect()
    }

    pub fn get_chunk_by_name(&self, name: &String) -> Option<&Chunk> {
        self.graph.node_weights().find(|c| c.filename().eq(name))
    }

    pub fn get_chunk_for_module(&self, module_id: &ModuleId) -> Option<&Chunk> {
        self.graph.node_weights().find(|c| c.has_module(module_id))
    }

    pub fn get_async_chunk_for_module(&self, module_id: &ModuleId) -> Option<&Chunk> {
        self.graph
            .node_weights()
            .find(|c| c.has_module(module_id) && matches!(c.chunk_type, ChunkType::Async))
    }

    // pub fn get_chunk_by_id(&self, id: &String) -> Option<&Chunk> {
    //     self.graph.node_weights().find(|c| c.id.id.eq(id))
    // }

    pub fn chunk(&self, chunk_id: &ChunkId) -> Option<&Chunk> {
        match self.id_index_map.get(chunk_id) {
            Some(idx) => self.graph.node_weight(*idx),
            None => None,
        }
    }

    pub fn mut_chunk(&mut self, chunk_id: &ChunkId) -> Option<&mut Chunk> {
        match self.id_index_map.get(chunk_id) {
            Some(idx) => self.graph.node_weight_mut(*idx),
            None => None,
        }
    }

    pub fn add_edge(&mut self, from: &ChunkId, to: &ChunkId) {
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph.add_edge(*from, *to, ());
    }

    pub fn remove_edge(&mut self, from: &ChunkId, to: &ChunkId) {
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph
            .remove_edge(self.graph.find_edge(*from, *to).unwrap());
    }

    pub fn chunk_names(&self) -> HashSet<String> {
        self.graph.node_weights().map(|c| c.filename()).collect()
    }

    pub fn full_hash(&self, module_graph: &ModuleGraph) -> u64 {
        let mut chunks = self.get_all_chunks();
        chunks.sort_by_key(|c| c.id.id.clone());

        let mut hasher: XxHash64 = Default::default();
        for c in chunks {
            hasher.write_u64(c.hash(module_graph))
        }
        hasher.finish()
    }

    pub fn sync_dependencies_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let idx = self.id_index_map.get(chunk_id).unwrap();
        let ret = self
            .graph
            .neighbors_directed(*idx, Direction::Outgoing)
            .filter(|idx| matches!(self.graph[*idx].chunk_type, ChunkType::Sync))
            .map(|idx| self.graph[idx].id.clone())
            .collect::<Vec<ChunkId>>();
        // The neighbors ordering is reversed, see https://github.com/petgraph/petgraph/issues/116,
        // so need to collect by reversed order
        ret.into_iter().rev().collect()
    }

    pub fn entry_dependencies_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let idx = self.id_index_map.get(chunk_id).unwrap();
        self.graph
            .neighbors_directed(*idx, Direction::Outgoing)
            .filter(|idx| matches!(self.graph[*idx].chunk_type, ChunkType::Entry(_, _, _)))
            .map(|idx| self.graph[idx].id.clone())
            .collect::<Vec<ChunkId>>()
    }

    pub fn dependents_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let idx = self.id_index_map.get(chunk_id).unwrap();
        self.graph
            .neighbors_directed(*idx, Direction::Incoming)
            .map(|idx| self.graph[idx].id.clone())
            .collect::<Vec<ChunkId>>()
    }

    pub fn entry_dependents_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let idx = self.id_index_map.get(chunk_id).unwrap();
        self.graph
            .neighbors_directed(*idx, Direction::Incoming)
            .filter(|idx| matches!(self.graph[*idx].chunk_type, ChunkType::Entry(_, _, _)))
            .map(|idx| self.graph[idx].id.clone())
            .collect::<Vec<ChunkId>>()
    }

    pub fn entry_ancestors_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let mut stack = vec![*self.id_index_map.get(chunk_id).unwrap()];
        let mut ret = vec![];
        let mut visited = vec![];

        while let Some(idx) = stack.pop() {
            if visited.contains(&idx.index()) {
                continue;
            }
            visited.push(idx.index());

            if matches!(self.graph[idx].chunk_type, ChunkType::Entry(_, _, _)) {
                ret.push(self.graph[idx].id.clone());
            }

            // continue to collect entry ancestors (include shared entry ancestors)
            stack.extend(self.graph.neighbors_directed(idx, Direction::Incoming));
        }
        ret
    }

    pub fn installable_descendants_chunk(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let mut dfs = Dfs::new(&self.graph, *self.id_index_map.get(chunk_id).unwrap());
        let mut ret = vec![];
        let mut visited = vec![];

        // petgraph dfs will visit all outgoing nodes by default
        while let Some(idx) = dfs.next(&self.graph) {
            if visited.contains(&idx.index()) {
                continue;
            }
            visited.push(idx.index());

            let chunk = &self.graph[idx];

            if !chunk.modules.is_empty()
                && matches!(chunk.chunk_type, ChunkType::Async | ChunkType::Sync)
            {
                ret.push(chunk.id.clone());
            }
        }
        ret
    }

    pub fn remove_chunk(&mut self, chunk_id: &ChunkId) {
        let idx = self.id_index_map.remove(chunk_id).unwrap();
        self.graph.remove_node(idx);
    }
}

impl Default for ChunkGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChunkGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut nodes = self
            .graph
            .node_weights()
            .map(|node| &node.id.id)
            .collect::<Vec<_>>();
        nodes.sort_by_key(|id| id.to_string());
        write!(f, "graph\n nodes:{:?}", &nodes)
    }
}
