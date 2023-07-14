use std::collections::{HashMap, HashSet};
use std::fmt;

use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use swc_ecma_ast::Module;

use crate::analyze_statement::analyze_statement;
use crate::statement::{StatementId, StatementType};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StatementGraphEdge {
    pub ident: HashSet<String>,
}

#[derive(Debug)]
pub struct StatementGraph {
    graph: StableDiGraph<StatementType, StatementGraphEdge>,
    id_index_map: HashMap<StatementId, NodeIndex>,
}

/**
 * 声明语句的依赖关系图
 */
impl StatementGraph {
    pub fn new(module: &Module) -> Self {
        let mut graph = StableDiGraph::new();
        let mut id_index_map = HashMap::new();

        // 只分析 body 顶层的声明语句
        for (index, statement) in module.body.iter().enumerate() {
            let node = graph.add_node(analyze_statement(index, statement));
            id_index_map.insert(index, node);
        }

        let mut graph = Self {
            graph,
            id_index_map,
        };

        graph.init_graph_edge();

        graph
    }

    pub fn empty() -> Self {
        Self {
            graph: StableDiGraph::new(),
            id_index_map: HashMap::new(),
        }
    }

    pub fn get_statements(&self) -> Vec<&StatementType> {
        self.graph.node_indices().map(|i| &self.graph[i]).collect()
    }

    pub fn get_statement(&self, id: &StatementId) -> &StatementType {
        let node = self.id_index_map.get(id).unwrap();
        &self.graph[*node]
    }

    pub fn get_dependencies(
        &self,
        id: &StatementId,
    ) -> Vec<(&StatementType, HashSet<String>, StatementId)> {
        let node = self.id_index_map.get(id).unwrap();
        self.graph
            .neighbors(*node)
            .map(|i| {
                let edge = self.graph.find_edge(*node, i).unwrap();
                let edge = self.graph.edge_weight(edge).unwrap();
                let statement = self.graph.node_weight(i).unwrap();
                let id = statement.get_id();
                (&self.graph[i], edge.ident.clone(), id)
            })
            .collect()
    }

    pub fn add_edge(&mut self, from: StatementId, to: StatementId, ident: HashSet<String>) {
        let from_node = self.id_index_map.get(&from).unwrap();
        let to_node = self.id_index_map.get(&to).unwrap();

        if let Some(edge) = self.graph.find_edge(*from_node, *to_node) {
            let edge = self.graph.edge_weight_mut(edge).unwrap();

            edge.ident.extend(ident);
            return;
        }

        self.graph
            .add_edge(*from_node, *to_node, StatementGraphEdge { ident });
    }

    fn init_graph_edge(&mut self) {
        let mut edges_to_add = Vec::new();
        for statement in self.get_statements() {
            for def_statement in self.get_statements() {
                let mut deps_ident = HashSet::new();
                for def_ident in def_statement.get_defined_ident() {
                    if let Some(used_ident) = statement.get_used_ident() {
                        if used_ident.contains(def_ident) {
                            deps_ident.insert(def_ident.clone());
                        }
                    }
                }
                if !deps_ident.is_empty() {
                    edges_to_add.push((statement.get_id(), def_statement.get_id(), deps_ident));
                }
            }
        }

        for (from, to, ident) in edges_to_add {
            self.add_edge(from, to, ident);
        }
    }
}

impl fmt::Display for StatementGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut nodes = self
            .graph
            .node_weights()
            .map(|node| {
                let id = node.get_id();
                match node {
                    StatementType::Import(import) => (
                        id,
                        format!(
                            "import {:?} - {:?}",
                            &stable_display_hashset(&import.defined_ident),
                            &import.is_self_executed
                        ),
                    ),
                    StatementType::Export(export) => (
                        id,
                        format!(
                            "export {:?} - {:?}",
                            &stable_display_hashset(&export.defined_ident),
                            &stable_display_hashset(&export.used_ident)
                        ),
                    ),
                    StatementType::Stmt {
                        id,
                        defined_ident,
                        used_ident,
                        is_self_executed,
                    } => (
                        *id,
                        format!(
                            "stmt {:?} - {:?} - {:?}",
                            &stable_display_hashset(defined_ident),
                            &stable_display_hashset(used_ident),
                            is_self_executed
                        ),
                    ),
                }
            })
            .collect::<Vec<_>>();
        let mut references = self
            .graph
            .edge_references()
            .map(|edge| {
                let source = &self.graph[edge.source()].get_id();
                let target = &self.graph[edge.target()].get_id();
                format!("{} -> {}", source, target)
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|id| id.0);
        references.sort_by_key(|id| id.to_string());
        write!(
            f,
            "graph\n nodes:{:?}\n \n references:{:?}",
            &nodes, &references
        )
    }
}

fn stable_display_hashset<T: ToString>(a: &HashSet<T>) -> Vec<&T> {
    let mut a = a.iter().collect::<Vec<_>>();
    a.sort_by_key(|id| id.to_string());
    a
}
