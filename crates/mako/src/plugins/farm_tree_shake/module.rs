use std::collections::{HashMap, HashSet};

use mako_core::swc_ecma_ast::{Module as SwcModule, ModuleItem};

use crate::module::{Module, ModuleId};
use crate::plugins::farm_tree_shake::statement_graph::{
    ExportInfo, ExportInfoMatch, ExportSpecifierInfo, ImportInfo, StatementGraph, StatementId,
};
use crate::tree_shaking::tree_shaking_module::ModuleSystem;

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
    Partial(HashSet<String>),
}

impl UsedExports {
    pub fn add_used_export(&mut self, used_export: &dyn ToString) -> bool {
        match self {
            UsedExports::All => {
                *self = UsedExports::All;
                false
            }
            UsedExports::Partial(self_used_exports) => {
                self_used_exports.insert(used_export.to_string())
            }
        }
    }

    pub fn use_all(&mut self) -> bool {
        match self {
            UsedExports::All => false,
            UsedExports::Partial(_) => {
                *self = UsedExports::All;
                // fixme : case partial used all exports
                true
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

#[derive(Clone, Debug)]
pub enum AllExports {
    Precise(HashSet<String>),
    Ambiguous(HashSet<String>),
}

impl AllExports {
    fn all_specifiers(&self) -> Vec<String> {
        match self {
            AllExports::Precise(ids) => ids
                .iter()
                .cloned()
                .filter(|id| id != "default")
                .collect::<Vec<_>>(),

            AllExports::Ambiguous(ids) => ids
                .iter()
                .cloned()
                .filter(|id| id != "default")
                .collect::<Vec<_>>(),
        }
    }

    pub fn to_all_specifier(&self) -> ExportSpecifierInfo {
        let sps = self.all_specifiers();

        match self {
            AllExports::Precise(_) => ExportSpecifierInfo::All(sps),
            AllExports::Ambiguous(_) => ExportSpecifierInfo::Ambiguous(sps),
        }
    }

    pub fn add_idents<I: IntoIterator<Item = String>>(&mut self, idents: I) {
        match self {
            AllExports::Precise(s) => s.extend(idents),
            AllExports::Ambiguous(s) => s.extend(idents),
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
    pub all_exports: AllExports,
    pub topo_order: usize,
    pub updated_ast: Option<SwcModule>,
}

impl TreeShakeModule {
    pub fn extends_exports(&mut self, to_extend: &AllExports) {
        match (&mut self.all_exports, to_extend) {
            (AllExports::Precise(me), AllExports::Precise(to_add)) => {
                me.extend(to_add.iter().cloned());
            }
            (AllExports::Ambiguous(me), AllExports::Precise(to_add)) => {
                me.extend(to_add.iter().cloned());
            }
            (AllExports::Precise(me), AllExports::Ambiguous(to_add)) => {
                me.extend(to_add.iter().cloned());

                self.all_exports = AllExports::Ambiguous(me.clone())
            }
            (AllExports::Ambiguous(me), AllExports::Ambiguous(to_add)) => {
                me.extend(to_add.iter().cloned());
            }
        }
    }

    pub fn new(module: &Module, order: usize) -> Self {
        let module_info = module.info.as_ref().unwrap();

        // 1. generate statement graph
        let mut module_system = ModuleSystem::CommonJS;
        let stmt_graph = match &module_info.ast {
            crate::module::ModuleAst::Script(module) => {
                let is_esm = module
                    .ast
                    .body
                    .iter()
                    .any(|s| matches!(s, ModuleItem::ModuleDecl(_)));
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
            UsedExports::Partial(Default::default())
        };

        Self {
            module_id: module.id.clone(),
            stmt_graph,
            used_exports,
            side_effects: module.side_effects,
            all_exports: match module_system {
                ModuleSystem::ESModule => AllExports::Precise(Default::default()),
                ModuleSystem::Custom | ModuleSystem::CommonJS => {
                    AllExports::Ambiguous(Default::default())
                }
            },
            module_system,
            topo_order: order,
            updated_ast: None,
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
            let used_idents: &mut HashSet<UsedIdent> =
                stmt_used_idents_map.entry(stmt_id).or_default();
            used_idents.insert(used_ident);
        }

        {
            for stmt in self.stmt_graph.stmts() {
                if stmt.is_self_executed {
                    stmt_used_idents_map.entry(stmt.id).or_default();

                    let dep_stmts = self.stmt_graph.dependencies(&stmt.id);

                    for (dep_stmt, referred_idents) in dep_stmts {
                        let used_idents = stmt_used_idents_map.entry(dep_stmt.id).or_default();
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
                            ExportSpecifierInfo::All(_) | ExportSpecifierInfo::Ambiguous(_) => {
                                used_idents.push((UsedIdent::ExportAll, export_info.stmt_id));
                            }
                        }
                    }
                }

                used_idents
            }
            // import {x,y,z} from "x"
            UsedExports::Partial(idents) => {
                let mut used_idents = vec![];

                for ident in idents {
                    // find the export info*s* that contains the ident

                    let mut export_infos = vec![];

                    for export_info in self.exports().into_iter() {
                        match export_info.matches_ident(ident) {
                            ExportInfoMatch::Matched => {
                                export_infos = vec![export_info];
                                break;
                            }
                            ExportInfoMatch::Unmatched => {}
                            ExportInfoMatch::Ambiguous => {
                                export_infos.push(export_info);
                            }
                        }
                    }

                    for export_info in export_infos {
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
                                ExportSpecifierInfo::All(exports_idents) => {
                                    let found = exports_idents.iter().find(|exported_ident| {
                                        is_ident_equal(ident, exported_ident)
                                    });

                                    if found.is_some() {
                                        used_idents.push((
                                            UsedIdent::InExportAll(ident.clone()),
                                            export_info.stmt_id,
                                        ));
                                    }
                                }
                                ExportSpecifierInfo::Ambiguous(_) => used_idents.push((
                                    UsedIdent::InExportAll(ident.clone()),
                                    export_info.stmt_id,
                                )),
                            }
                        }
                    }
                }

                used_idents
            }
        }
    }
}

pub fn is_ident_equal(ident1: &str, ident2: &str) -> bool {
    let split1 = ident1.split('#').collect::<Vec<_>>();
    let split2 = ident2.split('#').collect::<Vec<_>>();

    if split1.len() == 2 && split2.len() == 2 {
        split1[0] == split2[0] && split1[1] == split2[1]
    } else {
        split1[0] == split2[0]
    }
}
