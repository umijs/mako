use std::vec;

use crate::module::{Module, ModuleId};
use crate::statement::{ExportStatement, ImportStatement, StatementType};
use crate::statement_graph::StatementGraph;

#[derive(Debug, Clone)]
pub enum UsedExports {
    All,
    Partial(Vec<String>),
}

pub struct TreeShakingModule {
    pub id: ModuleId,
    pub used_exports: UsedExports,
    pub side_effects: bool,
    pub statement_graph: StatementGraph,
}

impl TreeShakingModule {
    pub fn new(module: &Module) -> Self {
        let ast = &module.info.as_ref().unwrap().ast;
        let statement_graph = match ast {
            crate::module::ModuleAst::Script(module) => StatementGraph::new(module),
            crate::module::ModuleAst::Css(_) => StatementGraph::empty(),
            crate::module::ModuleAst::None => StatementGraph::empty(),
        };

        let used_exports = if module.side_effects {
            UsedExports::All
        } else {
            UsedExports::Partial(vec![])
        };

        Self {
            id: module.id.clone(),
            used_exports,
            side_effects: module.side_effects,
            statement_graph,
        }
    }

    pub fn imports(&self) -> Vec<ImportStatement> {
        let mut imports: Vec<ImportStatement> = vec![];
        for statement in self.statement_graph.statements() {
            if let StatementType::Import(statement) = &statement {
                imports.push(statement.clone());
            }
        }
        imports
    }

    pub fn exports(&self) -> Vec<ExportStatement> {
        let mut exports: Vec<ExportStatement> = vec![];
        for statement in self.statement_graph.statements() {
            if let StatementType::Export(statement) = &statement {
                exports.push(statement.clone());
            }
        }
        exports
    }
}
