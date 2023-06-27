use std::{fmt, vec};

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
    statement_graph: StatementGraph,
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

    pub fn statements(&self) -> Vec<&StatementType> {
        self.statement_graph.statements()
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

impl fmt::Display for TreeShakingModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.statement_graph.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::TreeShakingModule;
    use crate::test_helper::create_mock_module;
    use crate::{assert_debug_snapshot, assert_display_snapshot};

    #[test]
    fn test_tree_shaking_module() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test"),
            r#"
import { x } from 'foo';
import 'bar';
const f0 = 1;
export const f1 = 1;
export const f2 = x;
"#,
        );
        let tree_shaking_module = TreeShakingModule::new(&module);
        assert_debug_snapshot!(&tree_shaking_module.statements());
        assert_eq!(tree_shaking_module.exports().len(), 2);
        assert_eq!(tree_shaking_module.imports().len(), 2);
        assert_display_snapshot!(&tree_shaking_module);
    }

    #[test]
    fn test_class_func() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test"),
            r#"
import { x } from 'foo';

export const f3 = x;
export const f1 = 1;

if (true) {
    const f1 = x;

    {
        const f2 = 1;
    }

    class Foo {
        constructor() {
            x;
            f1;
            f3;
        }
    }
}

"#,
        );
        let tree_shaking_module = TreeShakingModule::new(&module);
        assert_display_snapshot!(&tree_shaking_module);
    }
}
