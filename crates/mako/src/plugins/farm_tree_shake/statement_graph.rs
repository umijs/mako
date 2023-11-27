use std::collections::{HashMap, HashSet, VecDeque};

use mako_core::petgraph;
use mako_core::petgraph::stable_graph::NodeIndex;
use mako_core::swc_ecma_ast::{Module as SwcModule, ModuleItem};

pub(crate) mod analyze_imports_and_exports;
pub(crate) mod defined_idents_collector;
pub(crate) mod used_idents_collector;

use analyze_imports_and_exports::analyze_imports_and_exports;

use crate::plugins::farm_tree_shake::module::{is_ident_equal, UsedIdent};
use crate::plugins::farm_tree_shake::shake::strip_context;
use crate::plugins::farm_tree_shake::statement_graph::analyze_imports_and_exports::StatementInfo;

pub type StatementId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSpecifierInfo {
    Namespace(String),
    Named {
        local: String,
        imported: Option<String>,
    },
    Default(String),
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub source: String,
    pub specifiers: Vec<ImportSpecifierInfo>,
    pub stmt_id: StatementId,
}

// collect all exports and gathering them into a simpler structure
#[derive(Debug, Clone)]
pub enum ExportSpecifierInfo {
    // export * from 'foo';
    All(Vec<String>),
    // export { foo, bar, default as zoo } from 'foo';
    Named {
        local: String,
        exported: Option<String>,
    },
    // export default xxx;
    Default,
    // export * as foo from 'foo';
    Namespace(String),
    Ambiguous(Vec<String>),
}

impl ExportSpecifierInfo {
    pub fn to_idents(&self) -> Vec<String> {
        match self {
            ExportSpecifierInfo::All(_what) => {
                vec![]
            }
            ExportSpecifierInfo::Named { local, exported } => {
                if let Some(exp) = exported {
                    vec![strip_context(exp)]
                } else {
                    vec![strip_context(local)]
                }
            }
            ExportSpecifierInfo::Default => {
                vec!["default".to_string()]
            }
            ExportSpecifierInfo::Namespace(ns) => {
                vec![strip_context(ns)]
            }
            ExportSpecifierInfo::Ambiguous(_) => {
                vec![]
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportInfo {
    pub source: Option<String>,
    pub specifiers: Vec<ExportSpecifierInfo>,
    pub stmt_id: StatementId,
}

pub enum ExportInfoMatch {
    Matched,
    Unmatched,
    Ambiguous,
}

impl ExportInfo {
    pub fn matches_ident(&self, ident: &String) -> ExportInfoMatch {
        let mut res = ExportInfoMatch::Unmatched;

        for specifier in self.specifiers.iter() {
            match specifier {
                ExportSpecifierInfo::Default => {
                    if ident == "default" {
                        return ExportInfoMatch::Matched;
                    }
                }
                ExportSpecifierInfo::Named { local, exported } => {
                    let exported_ident = if let Some(exported) = exported {
                        exported
                    } else {
                        local
                    };

                    if is_ident_equal(ident, exported_ident) {
                        return ExportInfoMatch::Matched;
                    }
                }
                ExportSpecifierInfo::Namespace(ns) => {
                    if is_ident_equal(ident, ns) {
                        return ExportInfoMatch::Matched;
                    }
                }
                ExportSpecifierInfo::All(exported_idents) => {
                    let found = exported_idents.iter().find(|i| is_ident_equal(ident, i));

                    if found.is_some() {
                        return ExportInfoMatch::Matched;
                    }
                }
                ExportSpecifierInfo::Ambiguous(idents) => {
                    if idents.iter().any(|i| is_ident_equal(ident, i)) {
                        return ExportInfoMatch::Matched;
                    }

                    res = ExportInfoMatch::Ambiguous;
                }
            }
        }

        res
    }
}

#[derive(Debug)]
pub struct Statement {
    pub id: StatementId,
    pub import_info: Option<ImportInfo>,
    pub export_info: Option<ExportInfo>,
    pub defined_idents: HashSet<String>,
    pub used_idents: HashSet<String>,
    /// Use String to replace Ident as key, because Ident has position info and it will make hash map not work as expected,
    /// transform it to Ident.to_string() is exactly what we want
    pub defined_idents_map: HashMap<String, HashSet<String>>,
    pub is_self_executed: bool,
}

impl Statement {
    pub fn new(id: StatementId, stmt: &ModuleItem) -> Self {
        let StatementInfo {
            import_info,
            export_info,
            defined_idents,
            used_idents,
            defined_idents_map,
            is_self_executed,
        } = analyze_imports_and_exports(&id, stmt, None);

        // transform defined_idents_map from HashMap<Ident, Vec<Ident>> to HashMap<String, Ident> using ToString
        let defined_idents_map = defined_idents_map
            .into_iter()
            .map(|(key, value)| (key, value))
            .collect();

        Self {
            id,
            import_info,
            export_info,
            defined_idents,
            used_idents,
            defined_idents_map,
            is_self_executed,
        }
    }
}

pub struct StatementGraphEdge {
    pub idents: HashSet<String>,
}

pub struct StatementGraph {
    g: petgraph::graph::Graph<Statement, StatementGraphEdge>,
    id_index_map: HashMap<StatementId, NodeIndex>,
}

impl StatementGraph {
    pub fn new(module: &SwcModule, _side_effects_map: &HashMap<String, bool>) -> Self {
        let mut g = petgraph::graph::Graph::new();
        let mut id_index_map = HashMap::new();

        for (index, stmt) in module.body.iter().enumerate() {
            let statement = Statement::new(index, stmt);

            let node = g.add_node(statement);
            id_index_map.insert(index, node);
        }

        let mut graph = Self { g, id_index_map };
        let mut edges_to_add = Vec::new();

        for stmt in graph.stmts() {
            // find the statement that defines the ident
            for def_stmt in graph.stmts() {
                let mut deps_idents = HashSet::new();

                for di in &def_stmt.defined_idents {
                    if stmt.used_idents.contains(di) {
                        deps_idents.insert(di.clone());
                    }
                }

                if !deps_idents.is_empty() {
                    edges_to_add.push((stmt.id, def_stmt.id, deps_idents));
                }
            }
        }

        for (from, to, idents) in edges_to_add {
            graph.add_edge(from, to, idents);
        }

        graph
    }

    pub fn empty() -> Self {
        Self {
            g: petgraph::graph::Graph::new(),
            id_index_map: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, from: StatementId, to: StatementId, idents: HashSet<String>) {
        let from_node = self.id_index_map.get(&from).unwrap();
        let to_node = self.id_index_map.get(&to).unwrap();

        // if self.g contains edge, insert idents into edge
        if let Some(edge) = self.g.find_edge(*from_node, *to_node) {
            let edge = self.g.edge_weight_mut(edge).unwrap();

            edge.idents.extend(idents);
            return;
        }

        self.g
            .add_edge(*from_node, *to_node, StatementGraphEdge { idents });
    }

    pub fn stmt(&self, id: &StatementId) -> &Statement {
        let node = self.id_index_map.get(id).unwrap();
        &self.g[*node]
    }

    #[allow(dead_code)]
    pub fn stmt_mut(&mut self, id: &StatementId) -> &mut Statement {
        let node = self.id_index_map.get(id).unwrap();
        &mut self.g[*node]
    }

    pub fn dependencies(&self, id: &StatementId) -> Vec<(&Statement, HashSet<String>)> {
        let node = self.id_index_map.get(id).unwrap();
        self.g
            .neighbors(*node)
            .map(|i| {
                let edge = self.g.find_edge(*node, i).unwrap();
                let edge = self.g.edge_weight(edge).unwrap();
                (&self.g[i], edge.idents.clone())
            })
            .collect()
    }

    pub fn stmts(&self) -> Vec<&Statement> {
        self.g.node_indices().map(|i| &self.g[i]).collect()
    }

    #[allow(dead_code)]
    pub fn edges(&self) -> Vec<(&Statement, &Statement, &StatementGraphEdge)> {
        self.g
            .edge_indices()
            .map(|i| {
                let (from, to) = self.g.edge_endpoints(i).unwrap();
                let edge = self.g.edge_weight(i).unwrap();
                (&self.g[from], &self.g[to], edge)
            })
            .collect()
    }

    pub fn analyze_used_statements_and_idents(
        &self,
        used_exports: HashMap<StatementId, HashSet<UsedIdent>>,
    ) -> HashMap<StatementId, HashSet<String>> {
        let mut used_statements: HashMap<usize, HashSet<String>> = HashMap::new();

        // sort used_exports by statement id
        let mut used_exports: Vec<_> = used_exports.into_iter().collect();
        used_exports.sort_by(|a, b| a.0.cmp(&b.0));

        for (stmt_id, used_export_idents) in used_exports {
            let mut used_dep_idents = HashSet::new();
            let mut used_defined_idents = HashSet::new();
            let mut skip = false;

            for ident in used_export_idents {
                match ident {
                    UsedIdent::SwcIdent(i) => {
                        used_defined_idents.insert(i.to_string());
                        let dep_idents = self.stmt(&stmt_id).defined_idents_map.get(&i.to_string());

                        if let Some(dep_idents) = dep_idents {
                            used_dep_idents.extend(dep_idents.iter().map(|i| i.to_string()));
                        }
                    }
                    UsedIdent::Default => {
                        let stmt = self.stmt(&stmt_id);
                        used_dep_idents.extend(stmt.used_idents.iter().map(|i| i.to_string()));
                    }
                    UsedIdent::InExportAll(specifier) => {
                        // if used_statements already contains this statement, add specifier to it
                        if let Some(specifiers) = used_statements.get_mut(&stmt_id) {
                            specifiers.insert(specifier);
                        } else {
                            used_statements.insert(stmt_id, [specifier].into());
                        }
                        skip = true;
                    }
                    UsedIdent::ExportAll => {
                        used_statements.insert(stmt_id, ["*".to_string()].into());
                        skip = true;
                    }
                }
            }

            if skip {
                continue;
            }

            let mut stmts = VecDeque::from([(stmt_id, used_defined_idents, used_dep_idents)]);
            let mut visited = HashSet::new();

            let hash_stmt = |stmt_id: &StatementId, used_defined_idents: &HashSet<String>| {
                let mut sorted_idents =
                    used_defined_idents.iter().cloned().collect::<Vec<String>>();
                sorted_idents.sort();

                format!("{}:{}", stmt_id, sorted_idents.join(""))
            };

            while let Some((stmt_id, used_defined_idents, used_dep_idents)) = stmts.pop_front() {
                let hash = hash_stmt(&stmt_id, &used_defined_idents);

                // if stmt_id is already in used_statements, add used_defined_idents to it
                if let Some(idents) = used_statements.get_mut(&stmt_id) {
                    idents.extend(used_defined_idents);
                } else {
                    used_statements.insert(stmt_id, used_defined_idents);
                }

                if visited.contains(&hash) {
                    continue;
                }

                visited.insert(hash);

                let deps = self.dependencies(&stmt_id);

                for (dep_stmt, dep_idents) in deps {
                    if dep_idents.iter().any(|di| used_dep_idents.contains(di)) {
                        let mut dep_stmt_idents = HashSet::new();
                        let mut dep_used_defined_idents = HashSet::new();

                        for ident in &used_dep_idents {
                            if let Some(dep_idents) =
                                dep_stmt.defined_idents_map.get(&ident.to_string())
                            {
                                dep_used_defined_idents.insert(ident.to_string());
                                dep_stmt_idents.extend(dep_idents.clone());
                            } else {
                                // if dep_stmt.defined_idents contains ident, push it to dep_used_defined_idents
                                if let Some(find_defined_ident) = dep_stmt.defined_idents.get(ident)
                                {
                                    dep_used_defined_idents.insert(find_defined_ident.to_string());
                                }
                            }
                        }

                        // if dep_stmt is already in stmts, merge dep_stmt_idents
                        if let Some((_, used_dep_defined_idents, used_dep_idents)) =
                            stmts.iter_mut().find(|(id, _, _)| *id == dep_stmt.id)
                        {
                            used_dep_defined_idents.extend(dep_used_defined_idents);
                            used_dep_idents.extend(dep_stmt_idents);
                        } else {
                            stmts.push_back((
                                dep_stmt.id,
                                dep_used_defined_idents,
                                dep_stmt_idents,
                            ));
                        }
                    }
                }
            }
        }

        used_statements
    }
}
