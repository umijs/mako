use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;

use swc_core::common::SyntaxContext;
use swc_core::ecma::ast::{Module as SwcModule, ModuleItem};

use crate::module::{Module, ModuleId, ModuleSystem};
use crate::plugins::tree_shaking::statement_graph::{
    ExportInfo, ExportInfoMatch, ExportSource, ExportSpecifierInfo, ImportInfo, StatementGraph,
    StatementId,
};

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

impl Display for UsedIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            UsedIdent::SwcIdent(ident) => ident.to_string(),
            UsedIdent::Default => "default".to_string(),
            UsedIdent::InExportAll(ident) => ident.to_string(),
            UsedIdent::ExportAll => "*".to_string(),
        };
        write!(f, "{}", str)
    }
}

#[derive(Debug, Clone)]
pub enum UsedExports {
    All,
    Partial(HashSet<String>),
    ReferredPartial(HashSet<String>),
}

impl UsedExports {
    pub fn add_used_export(&mut self, used_export: &dyn ToString) -> bool {
        match self {
            UsedExports::All => {
                *self = UsedExports::All;
                false
            }
            UsedExports::Partial(self_used_exports)
            | UsedExports::ReferredPartial(self_used_exports) => {
                self_used_exports.insert(used_export.to_string())
            }
        }
    }

    pub fn use_all(&mut self) -> bool {
        match self {
            UsedExports::All => false,
            UsedExports::Partial(_) | UsedExports::ReferredPartial(_) => {
                *self = UsedExports::All;
                // fixme : case partial used all exports
                true
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            UsedExports::All | UsedExports::ReferredPartial(_) => false,
            UsedExports::Partial(self_used_exports) => self_used_exports.is_empty(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AllExports {
    Precise(HashSet<String>),
    Ambiguous(HashSet<String>),
}

impl Default for AllExports {
    fn default() -> Self {
        Self::Precise(Default::default())
    }
}

impl AllExports {
    fn all_specifiers(&self) -> Vec<String> {
        match self {
            AllExports::Precise(ids) => ids
                .iter()
                .filter(|&id| id != "default")
                .cloned()
                .collect::<Vec<_>>(),

            AllExports::Ambiguous(ids) => ids
                .iter()
                .filter(|&id| id != "default")
                .cloned()
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

    pub fn extends(&mut self, other: AllExports) {
        match self {
            AllExports::Precise(self_set) => match other {
                AllExports::Precise(other_set) => {
                    self_set.extend(other_set);
                }
                AllExports::Ambiguous(other_set) => {
                    let mut new_set = HashSet::new();
                    new_set.extend(self_set.drain());
                    new_set.extend(other_set);
                    *self = AllExports::Ambiguous(new_set);
                }
            },
            AllExports::Ambiguous(self_set) => match other {
                AllExports::Precise(other_set) => {
                    self_set.extend(other_set);
                }
                AllExports::Ambiguous(other_set) => {
                    self_set.extend(other_set);
                }
            },
        }
    }

    pub fn as_ambiguous(&mut self) {
        match self {
            AllExports::Precise(s) => {
                let mut new_set = HashSet::new();
                new_set.extend(s.drain());
                *self = AllExports::Ambiguous(new_set);
            }
            AllExports::Ambiguous(_) => {}
        }
    }
}

pub struct TreeShakeModule {
    pub module_id: ModuleId,
    pub side_effects: bool,
    pub described_side_effects: Option<bool>,
    pub stmt_graph: StatementGraph,
    // used exports will be analyzed when tree shaking
    used_exports: UsedExports,
    pub module_system: ModuleSystem,
    pub all_exports: AllExports,
    pub is_async: bool,
    pub topo_order: usize,
    pub updated_ast: Option<SwcModule>,
    pub side_effect_dep_sources: HashSet<String>,
    pub unresolved_ctxt: SyntaxContext,
}

impl TreeShakeModule {
    pub fn update_stmt_graph(&mut self, module: &SwcModule) {
        let stmt_graph = StatementGraph::new(module, self.unresolved_ctxt);

        self.stmt_graph = stmt_graph;
    }

    pub fn has_side_effect(&self) -> bool {
        if let Some(described_side_effects) = self.described_side_effects {
            if !described_side_effects {
                return false;
            }
        }

        self.side_effects
    }

    pub fn update_side_effect(&mut self) -> bool {
        let mut side_effect_stmts = vec![];

        if let Some(described_side_effects) = self.described_side_effects {
            if !described_side_effects {
                return false;
            }
        }

        self.stmt_graph.stmts().iter().for_each(|&s| {
            if s.is_self_executed {
                return;
            }
            if let Some(source) = s
                .export_info
                .as_ref()
                .and_then(|export_info| export_info.source.as_ref())
            {
                if self.side_effect_dep_sources.contains(source) {
                    side_effect_stmts.push(s.id);
                }
            }

            if let Some(&source) = s
                .import_info
                .as_ref()
                .map(|import_info| &import_info.source)
                .as_ref()
            {
                if self.side_effect_dep_sources.contains(source) {
                    side_effect_stmts.push(s.id);
                }
            }
        });

        side_effect_stmts.iter().for_each(|id| {
            self.stmt_graph.stmt_mut(id).is_self_executed = true;
        });

        let has_self_exec = self.stmt_graph.stmts().iter().any(|&s| s.is_self_executed);

        if has_self_exec {
            self.side_effects = true;
        }

        self.side_effects
    }

    pub fn use_all_exports(&mut self) -> bool {
        self.used_exports.use_all()
    }

    pub fn add_used_export(&mut self, used_export: Option<&dyn ToString>) -> bool {
        if let Some(used_export) = used_export {
            if self.side_effects {
                match &self.used_exports {
                    UsedExports::All | UsedExports::ReferredPartial(_) => {}
                    UsedExports::Partial(already_used) => {
                        self.used_exports = UsedExports::ReferredPartial(already_used.clone());
                    }
                }
            }
            self.used_exports.add_used_export(used_export)
        } else {
            self.use_module()
        }
    }

    fn use_module(&mut self) -> bool {
        match self.used_exports {
            UsedExports::All => {}
            UsedExports::Partial(ref mut used_exports) => {
                self.used_exports = UsedExports::ReferredPartial(used_exports.clone());
                return true;
            }
            UsedExports::ReferredPartial(_) => {}
        };

        false
    }

    pub fn not_used(&self) -> bool {
        self.used_exports.is_empty()
    }

    pub fn new(module: &Module, order: usize) -> Self {
        let module_info = module.info.as_ref().unwrap();

        let mut unresolved_ctxt = SyntaxContext::empty();
        // 1. generate statement graph
        let module_system = module_info.module_system.clone();
        let stmt_graph = match &module_info.ast {
            crate::module::ModuleAst::Script(module) => {
                let is_esm = module
                    .ast
                    .body
                    .iter()
                    .any(|s| matches!(s, ModuleItem::ModuleDecl(_)));
                if is_esm {
                    unresolved_ctxt = unresolved_ctxt.apply_mark(module.unresolved_mark);
                    StatementGraph::new(&module.ast, unresolved_ctxt)
                } else {
                    StatementGraph::empty()
                }
            }
            crate::module::ModuleAst::Css(_) => StatementGraph::empty(),
            crate::module::ModuleAst::None => StatementGraph::empty(),
        };

        let used_exports = if module.is_entry {
            UsedExports::All
        } else {
            UsedExports::Partial(Default::default())
        };

        Self {
            module_id: module.id.clone(),
            stmt_graph,
            used_exports,
            described_side_effects: module.info.as_ref().unwrap().described_side_effect(),
            side_effects: module_system != ModuleSystem::ESModule,
            side_effect_dep_sources: Default::default(),
            is_async: module.info.as_ref().unwrap().is_async,
            all_exports: match module_system {
                ModuleSystem::ESModule => AllExports::Precise(Default::default()),
                ModuleSystem::Custom | ModuleSystem::CommonJS => {
                    AllExports::Ambiguous(Default::default())
                }
            },
            module_system,
            topo_order: order,
            updated_ast: None,
            unresolved_ctxt,
        }
    }

    #[allow(dead_code)]
    pub fn imports(&self) -> Vec<ImportInfo> {
        let mut imports = vec![];

        for stmt in self.stmt_graph.stmts() {
            if let Some(import) = &stmt.import_info {
                imports.push(import.clone());
            }
        }

        imports
    }

    pub fn contains_exports_star(&self) -> bool {
        self.stmt_graph.stmts().into_iter().any(|stmt| {
            if let Some(export_info) = &stmt.export_info {
                if let Some(sp) = export_info.specifiers.first() {
                    return matches!(
                        sp,
                        ExportSpecifierInfo::All(_) | ExportSpecifierInfo::Ambiguous(_)
                    );
                }
            }
            false
        })
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

    pub fn used_statements(&self) -> BTreeMap<StatementId, HashSet<String>> {
        // 1. get used exports
        let used_exports_idents = self.used_exports_idents();
        let mut stmt_used_idents_map = BTreeMap::new();

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
                            ExportSpecifierInfo::Default(_) => {
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
            UsedExports::Partial(idents) | UsedExports::ReferredPartial(idents) => {
                let mut used_idents = vec![];

                for ident in idents {
                    // find the export info*s* that contains the ident
                    // in looped modules, there may be multiple export infos that contain the
                    // same ident
                    // eg: https://github.com/umijs/mako/issues/1273

                    let mut export_infos = vec![];

                    for export_info in self.exports().into_iter() {
                        let source: ExportSource = (&export_info).into();
                        let stmt_id = export_info.stmt_id;
                        match export_info.matches_ident(ident) {
                            ExportInfoMatch::Matched => {
                                export_infos.push((
                                    export_info,
                                    (ExportInfoMatch::Matched, source, stmt_id),
                                ));
                            }
                            ExportInfoMatch::Unmatched => {}
                            ExportInfoMatch::Ambiguous => {
                                export_infos.push((
                                    export_info,
                                    (ExportInfoMatch::Ambiguous, source, stmt_id),
                                ));
                            }
                        }
                    }

                    export_infos.sort_by_key(|(_, order)| order.clone());

                    if let Some((_, (matched, _, _))) = export_infos.first() {
                        if *matched == ExportInfoMatch::Matched {
                            export_infos.truncate(1)
                        }
                    }

                    for (export_info, _) in export_infos {
                        for sp in export_info.specifiers {
                            match sp {
                                ExportSpecifierInfo::Default(_) => {
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

pub fn is_ident_sym_equal(ident1: &str, ident2: &str) -> bool {
    let split1 = ident1.split('#').collect::<Vec<_>>();
    let split2 = ident2.split('#').collect::<Vec<_>>();

    split1[0] == split2[0]
}
