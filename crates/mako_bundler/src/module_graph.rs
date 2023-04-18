use std::collections::HashMap;

use crate::module::{Module, ModuleId};
use petgraph::{
    algo::toposort,
    graph::{DefaultIx, NodeIndex},
    stable_graph::StableDiGraph,
};

/**
 * Dependency = Module Graph Edge
 * 代码中的依赖关系就相当于有向图的边
 *
 * a.js
 * import b from './b'
 *
 * 则代表：
 * a -> b
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub source: String,
    pub resolve_type: ResolveType,
	/**
	 * import or export 的顺序，generate 的时候要根据这个顺序来生成
	 */
    pub order: usize,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ResolveType {
    Entry,
    Import,
    ExportNamed,
    ExportAll,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cycle<N>(N);

impl<N> Cycle<N> {
    /// Return a node id that participates in the cycle
    pub fn node_id(&self) -> N
    where
        N: Copy,
    {
        self.0
    }
}

pub struct ModuleGraph {
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
    graph: StableDiGraph<Module, Dependency>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            id_index_map: HashMap::new(),
            graph: StableDiGraph::new(),
        }
    }

    pub fn add_module(&mut self, module: Module) {
        let id = module.id.clone();
        let idx = self.graph.add_node(module);
        self.id_index_map.insert(id, idx);
    }

    pub fn add_dependency(&mut self, from: &ModuleId, to: &ModuleId, edge: Dependency) {
        // TODO: error handler
        let from = self.id_index_map.get(from).unwrap();
        let to = self.id_index_map.get(to).unwrap();
        self.graph.update_edge(*from, *to, edge);
    }

    pub fn get_module(&self, module_id: &ModuleId) -> Option<&Module> {
        let i = self.id_index_map.get(module_id);

        if let Some(i) = i {
            self.graph.node_weight(*i)
        } else {
            None
        }
    }

    pub fn get_module_mut(&mut self, module_id: &ModuleId) -> Option<&mut Module> {
        let i = self.id_index_map.get(module_id);

        if let Some(i) = i {
            self.graph.node_weight_mut(*i)
        } else {
            None
        }
    }

    /**
     * 对图进行拓扑排序
	 * TODO: 1. 针对 sideEffects 情况的处理，import 顺序需要按照 order 排序
     * TODO: 2. 针对成环情况下的友好处理
     */
    pub fn topo_sort(&mut self) -> Result<Vec<ModuleId>, Cycle<ModuleId>> {
        let orders = toposort(&self.graph, None);
        match orders {
            Ok(orders) => {
                let orders = orders
                    .into_iter()
                    .map(|idx| self.graph[idx].id.clone())
                    .collect();
                Ok(orders)
            }
            Err(err) => {
                let id = err.node_id();
                let id = self.graph[id].id.clone();
                Err(Cycle(id))
            }
        }
    }
}
