use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use crate::utils::bfs::{Bfs, NextResult};
use crate::{
    chunk::{Chunk, ChunkType},
    module::{Module, ModuleId},
};
use petgraph::prelude::EdgeRef;
use petgraph::visit::IntoEdgeReferences;
use petgraph::{
    graph::{DefaultIx, NodeIndex},
    stable_graph::StableDiGraph,
    Direction,
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
    // 依赖的模块在代码中的原始名称 eg react or ../components/box
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
    Require,
    DynamicImport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle<N> {
    cyclic: Vec<Vec<N>>,
}

pub struct ModuleGraph {
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
    graph: StableDiGraph<Module, Dependency>,
    entries: HashSet<ModuleId>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            id_index_map: HashMap::new(),
            graph: StableDiGraph::new(),
            entries: HashSet::new(),
        }
    }

    pub fn get_entry_modules(&self) -> HashSet<ModuleId> {
        self.entries.clone().into_iter().collect()
    }

    pub fn mark_entry_module(&mut self, module_id: &ModuleId) {
        self.entries.insert(module_id.clone());
    }

    pub fn add_module(&mut self, module: Module) {
        let id = module.id.clone();
        let idx = self.graph.add_node(module);
        self.id_index_map.insert(id, idx);
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) -> Module {
        let index = self
            .id_index_map
            .remove(module_id)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", module_id));
        self.graph.remove_node(index).unwrap()
    }

    pub fn get_or_add_module(&mut self, module_id: &ModuleId) -> &mut Module {
        if self.get_module_mut(module_id).is_none() {
            let module = Module::new(module_id.clone());
            self.add_module(module);
        }

        self.get_module_mut(module_id).unwrap()
    }

    pub fn update_module(&mut self, module: Module) {
        let id = module.id.clone();
        let index = self.id_index_map.get(&id).unwrap();
        self.graph[*index] = module;
    }

    pub fn add_dependency(&mut self, from: &ModuleId, to: &ModuleId, edge: Dependency) {
        let from = self
            .id_index_map
            .get(from)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", from));
        let to = self
            .id_index_map
            .get(to)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", to));
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

    pub fn has_module(&self, module_id: &ModuleId) -> bool {
        self.id_index_map.contains_key(module_id)
    }

    pub fn get_module_mut(&mut self, module_id: &ModuleId) -> Option<&mut Module> {
        let i = self.id_index_map.get(module_id);

        if let Some(i) = i {
            self.graph.node_weight_mut(*i)
        } else {
            None
        }
    }

    pub fn get_dependencies(&self, module_id: &ModuleId) -> Vec<(&ModuleId, &Dependency)> {
        let i = self
            .id_index_map
            .get(module_id)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", module_id));
        let mut edges = self
            .graph
            .neighbors_directed(*i, Direction::Outgoing)
            .detach();

        let mut deps: Vec<(&ModuleId, &Dependency)> = vec![];
        while let Some((edge_index, node_index)) = edges.next(&self.graph) {
            let dependency = self.graph.edge_weight(edge_index).unwrap();
            let module = self.graph.node_weight(node_index).unwrap();
            deps.push((&module.id, dependency));
        }
        deps.sort_by_key(|(_, dep)| dep.order);
        deps
    }

    pub fn get_modules(&self) -> Vec<ModuleId> {
        let mut modules = self
            .graph
            .node_indices()
            .map(|x| self.graph[x].id.clone())
            .collect::<Vec<_>>();
        // sort by module id
        modules.sort_by_key(|m| m.id.to_string());
        modules
    }

    /**
     * 对图进行拓扑排序
     * TODO: 1. 针对 sideEffects 情况的处理，import 顺序需要按照 order 排序
     * TODO: 2. 针对成环情况下的友好处理
     */
    pub fn topo_sort(&mut self) -> (Vec<ModuleId>, Cycle<ModuleId>) {
        fn dfs(
            entry: &ModuleId,
            graph: &ModuleGraph,
            stack: &mut Vec<ModuleId>,
            visited: &mut HashSet<ModuleId>,
            result: &mut Vec<ModuleId>,
            cyclic: &mut Vec<Vec<ModuleId>>,
        ) {
            // cycle detected
            if let Some(pos) = stack.iter().position(|m| m == entry) {
                cyclic.push(stack.clone()[pos..].to_vec());
                return;
            } else if visited.contains(entry) {
                // skip visited module
                return;
            }

            visited.insert(entry.clone());
            stack.push(entry.clone());

            let deps = graph.get_dependencies(entry);

            for (dep, _) in &deps {
                dfs(dep, graph, stack, visited, result, cyclic)
            }

            // visit current entry
            result.push(stack.pop().unwrap());
        }

        let mut result = vec![];
        let mut cyclic = vec![];
        let mut stack = vec![];

        // sort entries to make sure it is stable
        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort();

        let mut visited = HashSet::new();

        for entry in entries {
            let mut res = vec![];
            dfs(entry, self, &mut stack, &mut visited, &mut res, &mut cyclic);
            result.extend(res);
        }

        result.reverse();

        (result, Cycle { cyclic })
    }

    pub fn create_chunk_by_entry_module_id(
        &mut self,
        entry_module_id: &ModuleId,
        chunk_type: ChunkType,
    ) -> (Chunk, Vec<ModuleId>) {
        let mut dynamic_entries = vec![];
        let mut bfs = Bfs::new(VecDeque::from(vec![entry_module_id]), Default::default());
        let chunk = Chunk::new(entry_module_id.clone(), chunk_type);
        self.get_module_mut(entry_module_id)
            .unwrap()
            .chunks
            .insert(entry_module_id.clone());

        while !bfs.done() {
            match bfs.next_node() {
                NextResult::Visited => continue,
                NextResult::First(head) => {
                    for (dep_module_id, dep) in self.get_dependencies(head) {
                        if dep.resolve_type == ResolveType::DynamicImport {
                            dynamic_entries.push(dep_module_id.clone());
                        } else {
                            bfs.visit(dep_module_id);
                        }
                    }
                }
            }
        }

        (chunk, dynamic_entries)
    }
}

impl Default for ModuleGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ModuleGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut nodes = self
            .graph
            .node_weights()
            .into_iter()
            .map(|node| &node.id.id)
            .collect::<Vec<_>>();
        let mut references = self
            .graph
            .edge_references()
            .into_iter()
            .map(|edge| {
                let source = &self.graph[edge.source()].id.id;
                let target = &self.graph[edge.target()].id.id;
                format!("{} -> {}", source, target)
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|id| id.to_string());
        references.sort_by_key(|id| id.to_string());
        write!(
            f,
            "graph\n nodes:{:?} \n references:{:?}",
            &nodes, &references
        )
    }
}
