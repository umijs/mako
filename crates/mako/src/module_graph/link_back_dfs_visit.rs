use std::fmt::Debug;

use fixedbitset::FixedBitSet;
use hashlink::{linked_hash_map, LinkedHashMap};
use nohash_hasher::BuildNoHashHasher;
use petgraph::csr::IndexType;
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::stable_graph::StableDiGraph;
use petgraph::visit::{VisitMap, Visitable};
use petgraph::Direction;

/// This visitor is for css ordering, see test cases of css-merge-in-js and css-merge-in-css
#[derive(Clone, Debug)]
pub struct LinkBackDfs {
    pub stack: LinkedHashMap<usize, (), BuildNoHashHasher<usize>>,
    pub discovered: FixedBitSet,
    pub finished: FixedBitSet,
}

impl LinkBackDfs {
    pub fn new<N, E>(graph: &StableDiGraph<N, E>, start: NodeIndex) -> Self {
        let mut dfs = Self::empty(graph);
        dfs.move_to(start);
        dfs
    }

    pub fn empty<N, E>(graph: &StableDiGraph<N, E>) -> Self {
        LinkBackDfs {
            stack: LinkedHashMap::with_hasher(BuildNoHashHasher::default()),
            discovered: graph.visit_map(),
            finished: graph.visit_map(),
        }
    }

    pub fn move_to(&mut self, start: NodeIndex) {
        self.stack.clear();
        self.stack.insert(start.index(), ());
    }

    pub fn next<N, E, F>(
        &mut self,
        graph: &StableDiGraph<N, E>,
        execlude_edge: F,
    ) -> Option<NodeIndex>
    where
        F: Fn(EdgeIndex) -> bool,
    {
        while let Some((&nx, _)) = self.stack.iter().last().as_ref() {
            if self.discovered.visit(nx) {
                let mut nxs = Vec::<NodeIndex>::new();

                let mut walker = graph
                    .neighbors_directed(NodeIndex::new(nx), Direction::Outgoing)
                    .detach();
                while let Some((cex, cnx)) = walker.next(graph) {
                    if !execlude_edge(cex) {
                        nxs.push(cnx);
                    }
                }

                while let Some(cnx) = nxs.pop() {
                    if let linked_hash_map::Entry::Occupied(entry) = self.stack.entry(nx.index()) {
                        entry.cursor_mut().insert_before(cnx.index(), ());
                    }
                }
            } else {
                self.stack.pop_back();
                if self.finished.visit(nx) {
                    return Some(NodeIndex::new(nx));
                }
            }
        }
        None
    }
}
