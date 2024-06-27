use std::collections::{HashMap, HashSet};
use std::fmt;

use fixedbitset::FixedBitSet;
use petgraph::graph::{DefaultIx, NodeIndex};
use petgraph::prelude::{Dfs, EdgeRef};
use petgraph::stable_graph::{StableDiGraph, WalkNeighbors};
use petgraph::visit::IntoEdgeReferences;
use petgraph::Direction;
use tracing::debug;

use crate::module::{Dependencies, Dependency, Module, ModuleId, ResolveType};

#[derive(Debug)]
pub struct ModuleGraph {
    id_index_map: HashMap<ModuleId, NodeIndex<DefaultIx>>,
    pub graph: StableDiGraph<Module, Dependencies>,
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

    pub fn get_entry_modules(&self) -> Vec<ModuleId> {
        self.entries.iter().cloned().collect()
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

    pub fn modules(&self) -> Vec<&Module> {
        self.graph.node_weights().collect()
    }

    pub fn remove_module_and_deps(&mut self, module_id: &ModuleId) -> Module {
        let mut deps_module_ids = vec![];
        self.get_dependencies(module_id)
            .into_iter()
            .for_each(|(module_id, dep)| {
                deps_module_ids.push((module_id.clone(), dep.clone()));
            });
        for (to_module_id, dep) in deps_module_ids {
            self.remove_dependency(module_id, &to_module_id, &dep);
        }
        self.remove_module(module_id)
    }

    #[allow(dead_code)]
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

    pub fn remove_dependency(&mut self, from: &ModuleId, to: &ModuleId, dep: &Dependency) {
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
        let deps = self.graph.edge_weight_mut(edge).unwrap();
        deps.remove(dep);

        if deps.is_empty() {
            self.graph.remove_edge(edge);
        }
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
        let dep = self.graph.find_edge(*from, *to);
        if let Some(dep) = dep {
            let edges = self.graph.edge_weight_mut(dep).unwrap();
            edges.insert(edge);
        } else {
            let mut edges = Dependencies::new();
            edges.insert(edge);
            self.graph.update_edge(*from, *to, edges);
        }
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
            let dependencies = self.graph.edge_weight(edge_index).unwrap();
            let module = self.graph.node_weight(node_index).unwrap();
            dependencies.iter().for_each(|dep| {
                deps.push((&module.id, dep));
            })
        }
        deps.sort_by_key(|(_, dep)| dep.order);
        deps
    }

    pub fn get_dependents(&self, module_id: &ModuleId) -> Vec<(&ModuleId, &Dependency)> {
        let mut edges = self.get_edges(module_id, Direction::Incoming);
        let mut deps: Vec<(&ModuleId, &Dependency)> = vec![];
        while let Some((edge_index, node_index)) = edges.next(&self.graph) {
            let dependencies = self.graph.edge_weight(edge_index).unwrap();
            let module = self.graph.node_weight(node_index).unwrap();
            dependencies.iter().for_each(|dep| {
                deps.push((&module.id, dep));
            })
        }
        deps.sort_by_key(|(_, dep)| dep.order);
        deps
    }

    pub fn get_dependencies_info(
        &self,
        module_id: &ModuleId,
    ) -> Vec<(&ModuleId, &Dependency, bool)> {
        let mut edges = self.get_edges(module_id, Direction::Outgoing);
        let mut deps = vec![];
        while let Some((edge_index, node_index)) = edges.next(&self.graph) {
            let dependencies = self.graph.edge_weight(edge_index).unwrap();
            let module = self.graph.node_weight(node_index).unwrap();
            dependencies.iter().for_each(|dep| {
                let is_async = module
                    .info
                    .as_ref()
                    .is_some_and(|module_info| module_info.is_async);
                deps.push((&module.id, dep, is_async));
            })
        }
        deps.sort_by_key(|(_, dep, _)| dep.order);
        deps
    }

    pub fn dependant_dependencies(&self, module_id: &ModuleId) -> Vec<&Dependencies> {
        let mut edges = self.get_edges(module_id, Direction::Incoming);

        let mut deps = vec![];

        while let Some((edge_index, _)) = edges.next(&self.graph) {
            let dependencies = self.graph.edge_weight(edge_index).unwrap();
            deps.push(dependencies);
        }
        deps
    }

    pub fn dependant_module_ids(&self, module_id: &ModuleId) -> Vec<ModuleId> {
        let mut edges = self.get_edges(module_id, Direction::Incoming);
        let mut targets: Vec<ModuleId> = vec![];
        while let Some((_, node_index)) = edges.next(&self.graph) {
            let module = self.graph.node_weight(node_index).unwrap();
            targets.push(module.id.clone());
        }

        targets
    }

    pub fn dependence_module_ids(&self, module_id: &ModuleId) -> Vec<ModuleId> {
        let mut edges = self.get_edges(module_id, Direction::Outgoing);
        let mut targets: Vec<ModuleId> = vec![];
        while let Some((_, node_index)) = edges.next(&self.graph) {
            let module = self.graph.node_weight(node_index).unwrap();
            targets.push(module.id.clone());
        }

        targets
    }

    pub fn remove_dependency_module_by_source_and_resolve_type(
        &mut self,
        module_id: &ModuleId,
        source: &String,
        resolve_type: ResolveType,
    ) {
        let mut edges = self.get_edges(module_id, Direction::Outgoing);

        while let Some((edge_index, _node_index)) = edges.next(&self.graph) {
            let dependencies = self.graph.edge_weight_mut(edge_index).unwrap();

            if let Some(to_del_dep) = dependencies
                .iter()
                .position(|dep| *source == dep.source && dep.resolve_type.same_enum(&resolve_type))
            {
                dependencies.take(&dependencies.iter().nth(to_del_dep).unwrap().clone());

                if dependencies.is_empty() {
                    self.graph.remove_edge(edge_index);
                }
                return;
            }
        }
    }

    pub fn get_dependency_module_by_source(
        &self,
        module_id: &ModuleId,
        source: &String,
    ) -> Option<&ModuleId> {
        let deps = self.get_dependencies(module_id);
        for (module_id, dep) in deps {
            if *source == dep.source {
                return Some(module_id);
            }
        }
        debug!(
            "can not find module by source: {} in module {}",
            source, module_id.id
        );
        None
    }

    /**
     * 拓扑排序，得到成环依赖
     */
    pub fn toposort(&self) -> (Vec<ModuleId>, Vec<Vec<ModuleId>>) {
        fn dfs(
            entry: &ModuleId,
            graph: &ModuleGraph,
            stack: &mut Vec<ModuleId>,
            visited: &mut HashSet<ModuleId>,
            result: &mut Vec<ModuleId>,
            cyclic: &mut Vec<Vec<ModuleId>>,
        ) {
            if let Some(pos) = stack.iter().position(|m| m == entry) {
                cyclic.push(stack.clone()[pos..].to_vec());
                return;
            } else if visited.contains(entry) {
                return;
            }

            visited.insert(entry.clone());
            stack.push(entry.clone());

            let deps = graph.get_dependencies(entry);

            for (dep, _) in &deps {
                dfs(dep, graph, stack, visited, result, cyclic)
            }

            result.push(stack.pop().unwrap());
        }

        let mut result = vec![];
        let mut cyclic = vec![];
        let mut stack = vec![];

        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort();

        let mut visited = HashSet::new();

        for entry in entries {
            let mut res = vec![];
            dfs(entry, self, &mut stack, &mut visited, &mut res, &mut cyclic);

            result.extend(res);
        }

        result.reverse();

        (result, cyclic)
    }

    pub fn get_reference(&self) -> Vec<String> {
        let mut references = self
            .graph
            .edge_references()
            .map(|edge| {
                let source = &self.graph[edge.source()].id.id;
                let target = &self.graph[edge.target()].id.id;
                format!("{} -> {}", source, target)
            })
            .collect::<Vec<_>>();
        references.sort_by_key(|id| id.to_string());
        references
    }

    pub fn dfs(&self, start: &ModuleId) -> Dfs<NodeIndex, FixedBitSet> {
        Dfs::new(&self.graph, *self.id_index_map.get(start).unwrap())
    }
}

impl fmt::Display for ModuleGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut nodes = self
            .graph
            .node_weights()
            .map(|node| &node.id.id)
            .collect::<Vec<_>>();
        let references = self.get_reference();
        nodes.sort_by_key(|id| id.to_string());
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
