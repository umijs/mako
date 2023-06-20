use std::collections::{HashMap, HashSet};

use petgraph::stable_graph::NodeIndex;
use petgraph::Graph;
use swc_ecma_ast::Module;

use crate::analyze_statement::analyze_statement;
use crate::statement::{StatementId, StatementType};

pub struct StatementGraphEdge {
    pub ident: HashSet<String>,
}

pub struct StatementGraph {
    graph: Graph<StatementType, StatementGraphEdge>,
    id_index_map: HashMap<StatementId, NodeIndex>,
}

/**
 * 声明语句的依赖关系图
 */
impl StatementGraph {
    pub fn new(module: &Module) -> Self {
        let mut graph = Graph::new();
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
            graph: petgraph::graph::Graph::new(),
            id_index_map: HashMap::new(),
        }
    }

    pub fn statements(&self) -> Vec<&StatementType> {
        self.graph.node_indices().map(|i| &self.graph[i]).collect()
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
        for statement in self.statements() {
            for def_statement in self.statements() {
                let mut deps_indents = HashSet::new();
                for def_ident in def_statement.get_defined_ident() {
                    if let Some(used_ident) = statement.get_used_ident() {
                        if used_ident.contains(def_ident) {
                            deps_indents.insert(def_ident.clone());
                        }
                    }
                }
                if !deps_indents.is_empty() {
                    edges_to_add.push((statement.get_id(), def_statement.get_id(), deps_indents));
                }
            }
        }

        for (from, to, ident) in edges_to_add {
            self.add_edge(from, to, ident);
        }
    }
}
