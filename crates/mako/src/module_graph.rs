use std::collections::{HashMap, HashSet};
use std::fmt;

use petgraph::graph::{DefaultIx, NodeIndex};
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::{StableDiGraph, WalkNeighbors};
use petgraph::visit::IntoEdgeReferences;
use petgraph::Direction;

use crate::module::{Dependency, Module, ModuleId};

pub struct ModuleGraph {
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
    pub graph: StableDiGraph<Module, Dependency>,
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

    pub fn get_entry_modules(&self) -> Vec<&ModuleId> {
        self.entries.iter().collect()
    }

    pub fn add_module(&mut self, module: Module) {
        // TODO: module.id 能否用引用以减少内存占用？
        let id_for_map = module.id.clone();
        let id_for_entry = module.id.clone();
        let is_entry = module.is_entry;
        let idx = self.graph.add_node(module);
        self.id_index_map.insert(id_for_map, idx);
        if is_entry {
            self.entries.insert(id_for_entry);
        }
    }

    pub fn has_module(&self, module_id: &ModuleId) -> bool {
        self.id_index_map.contains_key(module_id)
    }

    pub fn get_module(&self, module_id: &ModuleId) -> Option<&Module> {
        self.id_index_map
            .get(module_id)
            .and_then(|i| self.graph.node_weight(*i))
    }

    pub fn get_modules(&self) -> Vec<&Module> {
        self.graph.node_weights().collect()
    }

    pub fn remove_module(&mut self, module_id: &ModuleId) -> Module {
        let index = self
            .id_index_map
            .remove(module_id)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", module_id));
        self.graph.remove_node(index).unwrap()
    }

    pub fn get_module_mut(&mut self, module_id: &ModuleId) -> Option<&mut Module> {
        self.id_index_map
            .get(module_id)
            .and_then(|i| self.graph.node_weight_mut(*i))
    }

    pub fn get_module_ids(&self) -> Vec<ModuleId> {
        self.graph
            .node_weights()
            .map(|node| node.id.clone())
            .collect()
    }

    pub fn replace_module(&mut self, module: Module) {
        let i = self
            .id_index_map
            .get(&module.id)
            .unwrap_or_else(|| panic!("module_id {:?} should in the module graph", module.id));
        self.graph[*i] = module;
    }

    #[allow(dead_code)]
    pub fn get_modules_mut(&mut self) -> Vec<&mut Module> {
        self.graph.node_weights_mut().collect()
    }

    pub fn remove_dependency(&mut self, from: &ModuleId, to: &ModuleId) {
        let from_index = self.id_index_map.get(from).unwrap_or_else(|| {
            panic!(
                r#"from node "{}" does not exist in the module graph when remove edge"#,
                from.id
            )
        });

        let to_index = self.id_index_map.get(to).unwrap_or_else(|| {
            panic!(
                r#"to node "{}" does not exist in the module graph when remove edge"#,
                to.id
            )
        });

        let edge = self
            .graph
            .find_edge(*from_index, *to_index)
            .unwrap_or_else(|| {
                panic!(
                    r#"edge "{}" -> "{}" does not exist in the module graph when remove edge"#,
                    from.id, to.id
                )
            });

        self.graph.remove_edge(edge);
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

    // 公共方法抽出, InComing 找 targets, Outing 找 dependencies
    fn get_edges(&self, module_id: &ModuleId, direction: Direction) -> WalkNeighbors<u32> {
        let i = self
            .id_index_map
            .get(module_id)
            .unwrap_or_else(|| panic!("module_id {:?} not found in the module graph", module_id));
        let edges = self.graph.neighbors_directed(*i, direction).detach();
        edges
    }

    pub fn get_dependencies(&self, module_id: &ModuleId) -> Vec<(&ModuleId, &Dependency)> {
        let mut edges = self.get_edges(module_id, Direction::Outgoing);
        let mut deps: Vec<(&ModuleId, &Dependency)> = vec![];
        while let Some((edge_index, node_index)) = edges.next(&self.graph) {
            let dependency = self.graph.edge_weight(edge_index).unwrap();
            let module = self.graph.node_weight(node_index).unwrap();
            deps.push((&module.id, dependency));
        }
        deps.sort_by_key(|(_, dep)| dep.order);
        deps
    }

    pub fn get_targets(&self, module_id: &ModuleId) -> Vec<ModuleId> {
        let mut edges = self.get_edges(module_id, Direction::Incoming);
        let mut targets = vec![];
        while let Some((_, node_index)) = edges.next(&self.graph) {
            let module = self.graph.node_weight(node_index).unwrap();
            targets.push(module.id.clone());
        }

        targets
    }
}

impl fmt::Display for ModuleGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut nodes = self
            .graph
            .node_weights()
            .map(|node| &node.id.id)
            .collect::<Vec<_>>();
        let mut references = self
            .graph
            .edge_references()
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

impl Default for ModuleGraph {
    fn default() -> Self {
        Self::new()
    }
}
