use std::collections::{HashMap, HashSet};
use std::{fmt, vec};

use tracing::debug;

use crate::module::{Module, ModuleId};
use crate::statement::{
    ExportSpecifier, ExportStatement, ImportStatement, StatementId, StatementType,
};
use crate::statement_graph::StatementGraph;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UsedIdent {
    /// Local ident
    SwcIdent(String),
    /// Default ident
    Default,
    /// This ident is used and may be exported from other module
    InExportAll(String),
    /// All idents is used and may be exported from other module
    ExportAll,
}

impl ToString for UsedIdent {
    fn to_string(&self) -> String {
        match self {
            UsedIdent::SwcIdent(ident) => ident.to_string(),
            UsedIdent::Default => "default".to_string(),
            UsedIdent::InExportAll(ident) => ident.to_string(),
            UsedIdent::ExportAll => "*".to_string(),
        }
    }
}

/**
 * 当前模块被用到的exports
 */
#[derive(Debug, Clone)]
pub enum UsedExports {
    All,
    Partial(Vec<String>),
}

impl UsedExports {
    pub fn add_used_export(&mut self, used_export: &dyn ToString) {
        match self {
            UsedExports::All => {
                *self = UsedExports::All;
            }
            UsedExports::Partial(self_used_exports) => {
                self_used_exports.push(used_export.to_string())
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            UsedExports::All => false,
            UsedExports::Partial(self_used_exports) => self_used_exports.is_empty(),
        }
    }
}

pub struct UsedIdentHashMap(HashMap<StatementId, HashSet<UsedIdent>>);
impl From<UsedIdentHashMap> for Vec<(StatementId, HashSet<UsedIdent>)> {
    fn from(val: UsedIdentHashMap) -> Self {
        let mut vec = val.0.into_iter().collect::<Vec<_>>();
        vec.sort_by_key(|a| a.0);
        vec
    }
}

#[derive(Debug)]
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
            crate::module::ModuleAst::Script(module) => StatementGraph::new(&module.ast),
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

    #[allow(dead_code)]
    fn get_statements(&self) -> Vec<&StatementType> {
        self.statement_graph.get_statements()
    }

    pub fn get_statement(&self, id: &StatementId) -> &StatementType {
        self.statement_graph.get_statement(id)
    }

    pub fn imports(&self) -> Vec<ImportStatement> {
        let mut imports: Vec<ImportStatement> = vec![];
        for statement in self.statement_graph.get_statements() {
            if let StatementType::Import(statement) = &statement {
                imports.push(statement.clone());
            }
        }
        imports
    }

    pub fn exports(&self) -> Vec<ExportStatement> {
        let mut exports: Vec<ExportStatement> = vec![];
        for statement in self.statement_graph.get_statements() {
            if let StatementType::Export(statement) = &statement {
                exports.push(statement.clone());
            }
        }
        exports
    }

    /**
     * 获取使用到的所有导出的 statement
     */
    pub fn get_used_export_statement(&self) -> UsedIdentHashMap {
        let used_exports_ident = self.get_used_export_ident();
        let mut stmt_used_ident_map: HashMap<StatementId, HashSet<UsedIdent>> = HashMap::new();

        for (used_ident, stmt_id) in used_exports_ident {
            let used_idents: &mut HashSet<UsedIdent> =
                stmt_used_ident_map.entry(stmt_id).or_default();
            used_idents.insert(used_ident);
        }

        for stmt in self.statement_graph.get_statements() {
            if stmt.get_is_self_executed() {
                stmt_used_ident_map.entry(stmt.get_id()).or_default();

                for (dep_statement, referred_idents) in
                    self.statement_graph.get_dependencies(&stmt.get_id())
                {
                    let used_idents = stmt_used_ident_map
                        .entry(dep_statement.get_id())
                        .or_default();
                    used_idents.extend(referred_idents.into_iter().map(UsedIdent::SwcIdent));
                }
            }
        }

        UsedIdentHashMap(stmt_used_ident_map)
    }

    /**
     * 当前模块内到处的 identifiers
     */
    pub fn get_used_export_ident(&self) -> Vec<(UsedIdent, usize)> {
        debug!("used_exports {:?}", &self.used_exports);
        match &self.used_exports {
            UsedExports::All => {
                // all exported identifiers are used
                let mut used_ident = vec![];

                for export_statement in self.exports() {
                    for sp in export_statement.info.specifiers {
                        match sp {
                            ExportSpecifier::Default => {
                                used_ident.push((UsedIdent::Default, export_statement.id));
                            }
                            ExportSpecifier::Named { local, .. } => {
                                used_ident.push((
                                    UsedIdent::SwcIdent(local.clone()),
                                    export_statement.id,
                                ));
                            }
                            ExportSpecifier::Namespace(ns) => {
                                used_ident
                                    .push((UsedIdent::SwcIdent(ns.clone()), export_statement.id));
                            }
                            ExportSpecifier::All(_) => {
                                used_ident.push((UsedIdent::ExportAll, export_statement.id));
                            }
                        }
                    }
                }

                used_ident
            }
            UsedExports::Partial(idents) => {
                let mut used_ident = vec![];

                for ident in idents {
                    let export_statements = self.exports().into_iter().find(|export_statement| {
                        export_statement.info.specifiers.iter().any(|sp| match sp {
                            ExportSpecifier::Default => ident == "default",
                            ExportSpecifier::Named { local, exported } => {
                                let exported_ident = if let Some(exported) = exported {
                                    exported
                                } else {
                                    local
                                };

                                Self::is_same_ident(ident, exported_ident)
                            }
                            ExportSpecifier::Namespace(ns) => Self::is_same_ident(ident, ns),
                            ExportSpecifier::All(_) => false,
                        })
                    });

                    if let Some(export_statement) = export_statements {
                        for sp in export_statement.info.specifiers {
                            match sp {
                                ExportSpecifier::Default => {
                                    if ident == "default" {
                                        used_ident.push((UsedIdent::Default, export_statement.id));
                                    }
                                }
                                ExportSpecifier::Named { local, exported } => {
                                    if let Some(exported) = exported {
                                        if Self::is_same_ident(ident, &exported) {
                                            used_ident.push((
                                                UsedIdent::SwcIdent(local.clone()),
                                                export_statement.id,
                                            ));
                                        }
                                    } else if Self::is_same_ident(ident, &local) {
                                        used_ident.push((
                                            UsedIdent::SwcIdent(local.clone()),
                                            export_statement.id,
                                        ));
                                    }
                                }
                                ExportSpecifier::Namespace(ns) => {
                                    if Self::is_same_ident(ident, &ns) {
                                        used_ident.push((
                                            UsedIdent::SwcIdent(ns.clone()),
                                            export_statement.id,
                                        ));
                                    }
                                }
                                ExportSpecifier::All(_) => unreachable!(),
                            }
                        }
                    } else {
                        for export_statement in self.exports() {
                            if export_statement
                                .info
                                .specifiers
                                .iter()
                                .any(|sp| matches!(sp, ExportSpecifier::All(_)))
                            {
                                used_ident.push((
                                    UsedIdent::InExportAll(ident.to_string()),
                                    export_statement.id,
                                ));
                            }
                        }
                    }
                }

                used_ident
            }
        }
    }

    fn is_same_ident(ident1: &str, ident2: &str) -> bool {
        let split1 = ident1.split('#').collect::<Vec<_>>();
        let split2 = ident2.split('#').collect::<Vec<_>>();

        if split1.len() == 2 && split2.len() == 2 {
            split1[0] == split2[0] && split1[1] == split2[1]
        } else {
            split1[0] == split2[0]
        }
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
        assert_debug_snapshot!(&tree_shaking_module.get_statements());
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
