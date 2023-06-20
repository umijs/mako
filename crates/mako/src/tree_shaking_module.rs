use std::vec;

use crate::module::{Module, ModuleId};
use crate::statement::{ExportInfo, ImportInfo, StatementType};
use crate::statement_graph::StatementGraph;

pub struct TreeShakingModule {
    pub id: ModuleId,
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
        Self {
            id: module.id.clone(),
            side_effects: false,
            statement_graph,
        }
    }

    pub fn imports(&self) -> Vec<ImportInfo> {
        let mut imports: Vec<ImportInfo> = vec![];
        for statement in self.statement_graph.statements() {
            if let StatementType::Import {
                id: _,
                info,
                is_self_executed: _,
                defined_ident: _,
            } = &statement
            {
                imports.push(info.clone());
            }
        }
        imports
    }

    pub fn exports(&self) -> Vec<ExportInfo> {
        let mut exports: Vec<ExportInfo> = vec![];
        for statement in self.statement_graph.statements() {
            if let StatementType::Export {
                id: _,
                info,
                defined_ident: _,
                used_ident: _,
            } = &statement
            {
                exports.push(info.clone());
            }
        }
        exports
    }
}
