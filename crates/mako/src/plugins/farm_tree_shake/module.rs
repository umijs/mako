use std::collections::{HashMap, HashSet};

use crate::module::{Module, ModuleId};
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportSpecifierInfo, ImportInfo, StatementGraph, StatementId,
};
use crate::tree_shaking_module::ModuleSystem;

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

pub struct TreeShakeModule {
    pub module_id: ModuleId,
    pub side_effects: bool,
    pub stmt_graph: StatementGraph,
    // used exports will be analyzed when tree shaking
    pub used_exports: UsedExports,
    pub module_system: ModuleSystem,
}

impl TreeShakeModule {
    pub fn new(module: &Module) -> Self {
        let module_info = module.info.as_ref().unwrap();

        // 1. generate statement graph
        let mut module_system = ModuleSystem::CommonJS;
        let stmt_graph = match &module_info.ast {
            crate::module::ModuleAst::Script(module) => {
                let is_esm = module
                    .ast
                    .body
                    .iter()
                    .any(|s| matches!(s, swc_ecma_ast::ModuleItem::ModuleDecl(_)));
                if is_esm {
                    module_system = ModuleSystem::ESModule;
                    StatementGraph::new(&module.ast)
                } else {
                    StatementGraph::empty()
                }
            }
            crate::module::ModuleAst::Css(_) => {
                module_system = ModuleSystem::Custom;
                StatementGraph::empty()
            }
            crate::module::ModuleAst::None => {
                module_system = ModuleSystem::Custom;
                StatementGraph::empty()
            }
        };

        // 2. set default used exports
        let used_exports = if module.side_effects {
            UsedExports::All
        } else {
            UsedExports::Partial(vec![])
        };

        Self {
            module_id: module.id.clone(),
            stmt_graph,
            used_exports,
            side_effects: module.side_effects,
            module_system,
        }
    }

    pub fn imports(&self) -> Vec<ImportInfo> {
        let mut imports = vec![];

        for stmt in self.stmt_graph.stmts() {
            if let Some(import) = &stmt.import_info {
                imports.push(import.clone());
            }
        }

        imports
    }

    pub fn exports(&self) -> Vec<ExportInfo> {
        let mut exports = vec![];

        for stmt in self.stmt_graph.stmts() {
            if let Some(export) = &stmt.export_info {
                exports.push(export.clone());
            }
        }

        exports
    }

    pub fn used_statements(&self) -> HashMap<StatementId, HashSet<String>> {
        // 1. get used exports
        let used_exports_idents = self.used_exports_idents();
        let mut stmt_used_idents_map = HashMap::new();

        for (used_ident, stmt_id) in used_exports_idents {
            let used_idents = stmt_used_idents_map
                .entry(stmt_id)
                .or_insert(HashSet::new());
            used_idents.insert(used_ident);
        }

        {
            for stmt in self.stmt_graph.stmts() {
                if stmt.is_self_executed {
                    stmt_used_idents_map
                        .entry(stmt.id)
                        .or_insert(HashSet::new());

                    let dep_stmts = self.stmt_graph.dependencies(&stmt.id);

                    for (dep_stmt, referred_idents) in dep_stmts {
                        let used_idents = stmt_used_idents_map
                            .entry(dep_stmt.id)
                            .or_insert(HashSet::new());
                        used_idents.extend(referred_idents.into_iter().map(UsedIdent::SwcIdent));
                    }
                    // stmt.used_idents.iter().for_each(|used_ident| {
                    //   // find the defined ident
                    //   stmt_used_idents_map
                    //   .entry(stmt.id)
                    //   .or_insert(HashSet::new());

                    //   for stmt_inner in self.stmt_graph.stmts() {
                    //     if stmt_inner.id == stmt.id {
                    //       continue;
                    //     }

                    //     if stmt_inner
                    //       .defined_idents_map
                    //       .contains_key(&used_ident.to_string())
                    //       || stmt_inner
                    //         .defined_idents
                    //         .iter()
                    //         .any(|ident| ident.to_string() == used_ident.to_string())
                    //     {
                    //       let used_idents = stmt_used_idents_map
                    //         .entry(stmt_inner.id)
                    //         .or_insert(HashSet::new());
                    //       used_idents.insert(UsedIdent::SwcIdent(used_ident.clone()));
                    //     }
                    //   }
                    // });
                }
            }
        }

        // 2. analyze used statements starting from used exports

        // println!("before {:?}", self.module_id);
        // dbg!(&used);
        //
        // for imp in self.imports() {
        //     if imp.source.contains("@swc/helpers") {
        //         used.entry(imp.stmt_id)
        //             .or_insert(HashSet::new());
        //     }
        // }
        //
        // dbg!(&used);

        self.stmt_graph
            .analyze_used_statements_and_idents(stmt_used_idents_map)
    }

    pub fn used_exports_idents(&self) -> Vec<(UsedIdent, StatementId)> {
        match &self.used_exports {
            UsedExports::All => {
                // all exported identifiers are used
                let mut used_idents = vec![];

                for export_info in self.exports() {
                    for sp in export_info.specifiers {
                        match sp {
                            ExportSpecifierInfo::Default => {
                                used_idents.push((UsedIdent::Default, export_info.stmt_id));
                            }
                            ExportSpecifierInfo::Named { local, .. } => {
                                used_idents.push((
                                    UsedIdent::SwcIdent(local.clone()),
                                    export_info.stmt_id,
                                ));
                            }
                            ExportSpecifierInfo::Namespace(ns) => {
                                used_idents
                                    .push((UsedIdent::SwcIdent(ns.clone()), export_info.stmt_id));
                            }
                            ExportSpecifierInfo::All(_) => {
                                used_idents.push((UsedIdent::ExportAll, export_info.stmt_id));
                            }
                        }
                    }
                }

                used_idents
            }
            UsedExports::Partial(idents) => {
                let mut used_idents = vec![];

                for ident in idents {
                    // find the export info that contains the ident
                    let export_info = self.exports().into_iter().find(|export_info| {
                        export_info.specifiers.iter().any(|sp| match sp {
                            ExportSpecifierInfo::Default => ident == "default",
                            ExportSpecifierInfo::Named { local, exported } => {
                                let exported_ident = if let Some(exported) = exported {
                                    exported
                                } else {
                                    local
                                };

                                is_ident_equal(ident, exported_ident)
                            }
                            ExportSpecifierInfo::Namespace(ns) => is_ident_equal(ident, ns),
                            ExportSpecifierInfo::All(_) => {
                                /* Deal with All later */
                                false
                            }
                        })
                    });

                    if let Some(export_info) = export_info {
                        for sp in export_info.specifiers {
                            match sp {
                                ExportSpecifierInfo::Default => {
                                    if ident == "default" {
                                        used_idents.push((UsedIdent::Default, export_info.stmt_id));
                                    }
                                }
                                ExportSpecifierInfo::Named { local, exported } => {
                                    if let Some(exported) = exported {
                                        if is_ident_equal(ident, &exported) {
                                            used_idents.push((
                                                UsedIdent::SwcIdent(local.clone()),
                                                export_info.stmt_id,
                                            ));
                                        }
                                    } else if is_ident_equal(ident, &local) {
                                        used_idents.push((
                                            UsedIdent::SwcIdent(local.clone()),
                                            export_info.stmt_id,
                                        ));
                                    }
                                }
                                ExportSpecifierInfo::Namespace(ns) => {
                                    if is_ident_equal(ident, &ns) {
                                        used_idents.push((
                                            UsedIdent::SwcIdent(ns.clone()),
                                            export_info.stmt_id,
                                        ));
                                    }
                                }
                                ExportSpecifierInfo::All(_) => unreachable!(),
                            }
                        }
                    } else {
                        // if export info is not found, and there are ExportSpecifierInfo::All, then the ident may be exported by `export * from 'xxx'`
                        for export_info in self.exports() {
                            if export_info
                                .specifiers
                                .iter()
                                .any(|sp| matches!(sp, ExportSpecifierInfo::All(_)))
                            {
                                let stmt_id = export_info.stmt_id;
                                used_idents
                                    .push((UsedIdent::InExportAll(ident.to_string()), stmt_id));
                            }
                        }
                    }
                }

                used_idents
            }
        }
    }
}

fn is_ident_equal(ident1: &String, ident2: &String) -> bool {
    let split1 = ident1.split('#').collect::<Vec<_>>();
    let split2 = ident2.split('#').collect::<Vec<_>>();

    if split1.len() == 2 && split2.len() == 2 {
        split1[0] == split2[0] && split1[1] == split2[1]
    } else {
        split1[0] == split2[0]
    }
}
