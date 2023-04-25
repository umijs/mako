use std::collections::HashMap;

use crate::{chunk::Chunk, module::ModuleId};
use petgraph::stable_graph::{DefaultIx, NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use petgraph::visit::IntoEdgeReferences;

pub struct ChunkGraph {
    graph: StableDiGraph<Chunk, ()>,
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
}

impl Default for ChunkGraph {
    fn default() -> Self {
        Self::new()
    }
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
        if self.id_index_map.contains_key(&chunk_id) {
            panic!("chunk already exists: {:?}", &chunk_id);
        }
        let node_index = self.graph.add_node(chunk);
        self.id_index_map.insert(chunk_id, node_index);
    }

    pub fn add_edge(&mut self, from: &ModuleId, to: &ModuleId) {
        let from_node_index = self.id_index_map.get(from).unwrap();
        let to_node_index = self.id_index_map.get(to).unwrap();
        self.graph.add_edge(*from_node_index, *to_node_index, ());
    }

    pub fn print_graph(&self) {
        println!("digraph {{\n nodes:");

        for node in self.graph.node_weights() {
            println!("  \"{}\";", &node.id.id);
        }

        println!("\nedges:");

        for edge in self.graph.edge_references() {
            let source = &self.graph[edge.source()].id.id;
            let target = &self.graph[edge.target()].id.id;
            println!("  \"{}\" -> \"{}\";", source, target);
        }

        println!("}}");
    }
}
