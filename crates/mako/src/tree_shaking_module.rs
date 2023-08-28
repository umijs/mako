use std::collections::{HashMap, HashSet};
use std::{fmt, vec};

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
                let used_export = used_export.to_string();

                if !self_used_exports.contains(&used_export) {
                    self_used_exports.push(used_export)
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            UsedExports::All => false,
            UsedExports::Partial(self_used_exports) => self_used_exports.is_empty(),
        }
    }

    pub fn contains(&self, used_export: &dyn ToString) -> bool {
        match self {
            UsedExports::All => true,
            UsedExports::Partial(self_used_exports) => {
                self_used_exports.contains(&used_export.to_string())
            }
        }
    }
}

#[derive(Debug)]
pub struct UsedIdentHashMap(HashMap<StatementId, HashSet<UsedIdent>>);
impl From<UsedIdentHashMap> for Vec<(StatementId, HashSet<UsedIdent>)> {
    fn from(val: UsedIdentHashMap) -> Self {
        let mut vec: Vec<(usize, HashSet<UsedIdent>)> = val.0.into_iter().collect::<Vec<_>>();
        vec.sort_by_key(|a| a.0);
        vec
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ModuleSystem {
    CommonJS,
    ESModule,
    Custom,
}

#[derive(Debug)]
pub struct TreeShakingModule {
    pub id: ModuleId,
    pub used_exports: UsedExports,
    pub side_effects: bool,
    pub module_system: ModuleSystem,
    statement_graph: StatementGraph,
}

#[derive(Debug)]
pub struct UsedReExportHashMap(HashMap<StatementId, Option<Vec<StatementId>>>);

impl UsedReExportHashMap {
    pub fn is_all_re_export(&self) -> bool {
        self.0.iter().all(|(_, v)| v.is_some())
    }
    pub fn get_all_used_statement_ids(&self) -> HashSet<StatementId> {
        let mut ids = HashSet::new();
        self.0
            .iter()
            .for_each(|(export_statement_id, import_statement)| {
                ids.insert(*export_statement_id);

                if let Some(import_statement) = import_statement {
                    import_statement.iter().for_each(|import_statement_id| {
                        ids.insert(*import_statement_id);
                    })
                }
            });
        ids
    }
}

impl TreeShakingModule {
    pub fn new(module: &Module) -> Self {
        let (module_system, statement_graph) = init_statement_graph(module);

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
            module_system,
        }
    }

    pub fn update_statement(&mut self, module: &Module) {
        let (_, statement_graph) = init_statement_graph(module);
        self.statement_graph = statement_graph;
    }

    #[allow(dead_code)]
    pub fn get_statements(&self) -> Vec<&StatementType> {
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

    pub fn get_used_re_exports(&self) -> UsedReExportHashMap {
        let mut used_export_statement_hashmap = HashMap::new();
        let mut keys = HashSet::new();
        self.get_used_export_ident()
            .into_iter()
            .for_each(|(_, id)| {
                used_export_statement_hashmap.entry(id).or_default();
                keys.insert(id);
            });

        for statement_id in keys {
            let statement = self.get_statement(&statement_id);
            if let StatementType::Export(statement) = &statement {
                // export from '.xxx';
                if statement.info.source.is_some() {
                    used_export_statement_hashmap.insert(statement.id, Some(vec![]));
                    continue;
                }

                let deps = self.statement_graph.get_dependencies(&statement.id);

                // import { f } from 'xxx'
                // export { f }
                let all_deps_is_import = deps
                    .iter()
                    .all(|(dep_statement, ..)| matches!(dep_statement, StatementType::Import(_)));
                if all_deps_is_import {
                    let import_statements = deps
                        .iter()
                        .map(|(dep_statement, ..)| {
                            if let StatementType::Import(import_statement) = dep_statement {
                                import_statement.id
                            } else {
                                unreachable!()
                            }
                        })
                        .collect::<Vec<_>>();
                    used_export_statement_hashmap.insert(statement.id, Some(import_statements));
                    continue;
                }

                used_export_statement_hashmap.insert(statement.id, None);
            } else {
                unreachable!();
            }
        }
        UsedReExportHashMap(used_export_statement_hashmap)
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

        for (used_ident, stmt_id) in &used_exports_ident {
            let used_idents: &mut HashSet<UsedIdent> =
                stmt_used_ident_map.entry(*stmt_id).or_default();
            used_idents.insert(used_ident.clone());
        }
        for stmt in self.statement_graph.get_statements() {
            // 当前 statement 是自执行，或者当前 statement 已经被使用到了
            if stmt.get_is_self_executed()
                || matches!(stmt, StatementType::Stmt { .. })
                || (used_exports_ident
                    .iter()
                    .any(|(_, id)| *id == stmt.get_id()))
            {
                let mut visited = HashSet::new();
                self.analyze_statement_used_ident(&mut stmt_used_ident_map, stmt, &mut visited);
            }
        }
        UsedIdentHashMap(stmt_used_ident_map)
    }

    fn analyze_statement_used_ident(
        &self,
        stmt_used_ident_map: &mut HashMap<usize, HashSet<UsedIdent>>,
        stmt: &StatementType,
        visited: &mut HashSet<usize>,
    ) {
        if visited.contains(&stmt.get_id()) {
            return;
        }
        visited.insert(stmt.get_id());

        stmt_used_ident_map.entry(stmt.get_id()).or_default();
        for (dep_statement, referred_idents, ..) in
            self.statement_graph.get_dependencies(&stmt.get_id())
        {
            let used_idents = stmt_used_ident_map
                .entry(dep_statement.get_id())
                .or_default();
            used_idents.extend(referred_idents.into_iter().map(UsedIdent::SwcIdent));

            self.analyze_statement_used_ident(stmt_used_ident_map, dep_statement, visited);
        }
    }

    /**
     * 当前模块内到处的 identifiers
     */
    pub fn get_used_export_ident(&self) -> Vec<(UsedIdent, StatementId)> {
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
                            ExportSpecifier::All => {
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
                            ExportSpecifier::All => false,
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
                                                UsedIdent::SwcIdent(exported.clone()),
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
                                ExportSpecifier::All => unreachable!(),
                            }
                        }
                    } else {
                        for export_statement in self.exports() {
                            if export_statement
                                .info
                                .specifiers
                                .iter()
                                .any(|sp| matches!(sp, ExportSpecifier::All))
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

fn init_statement_graph(module: &Module) -> (ModuleSystem, StatementGraph) {
    let ast = &module.info.as_ref().unwrap().ast;

    let mut module_system = ModuleSystem::CommonJS;
    let statement_graph = match ast {
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
    (module_system, statement_graph)
}

impl fmt::Display for TreeShakingModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.statement_graph.fmt(f)
    }
}

// FIXME: 找一个靠谱的方式
pub fn should_skip(url: &str) -> bool {
    vec![
        "@swc/helpers/esm/_interop_require_default",
        "@swc/helpers/esm/_interop_require_wildcard",
        "@swc/helpers/esm/_export_star",
        "@swc/helpers/_/_interop_require_default",
        "@swc/helpers/_/_interop_require_wildcard",
        "@swc/helpers/_/_export_star",
    ]
    .into_iter()
    .any(|id| url.contains(id))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use super::TreeShakingModule;
    use crate::test_helper::create_mock_module;
    use crate::tree_shaking_module::{UsedExports, UsedIdent};
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

    #[test]
    fn test_used_export_include_others() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test.tsx"),
            r#"
export function proxyWithPersist() {
    return 1;
}
// 声明语句所产生的变量被使用到
const todos = proxyWithPersist();

export function Todos() {
    console.log(todos);
}


"#,
        );
        let mut tree_shaking_module = TreeShakingModule::new(&module);
        tree_shaking_module.used_exports.add_used_export(&"Todos");
        let used: Vec<(usize, HashSet<UsedIdent>)> =
            tree_shaking_module.get_used_export_statement().into();
        assert_debug_snapshot!(&used);
    }

    #[test]
    fn test_used_export_include_others_2() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test.tsx"),
            r#"
export function foo() {}

// default 导出依赖别的函数
export default function bar() {
    foo();
}
"#,
        );
        let mut tree_shaking_module = TreeShakingModule::new(&module);
        tree_shaking_module.used_exports.add_used_export(&"default");
        let used: Vec<(usize, HashSet<UsedIdent>)> =
            tree_shaking_module.get_used_export_statement().into();
        assert_debug_snapshot!(&used);
    }

    #[test]
    fn test_used_export_include_others_loop() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test.tsx"),
            r#"
// 多套一层函数
export function compile (value) {
    return console.log(parse())
}

export function parse () {
    ruleset();
}

export function ruleset () {

}

"#,
        );
        let mut tree_shaking_module = TreeShakingModule::new(&module);
        tree_shaking_module.used_exports.add_used_export(&"compile");
        let used: Vec<(usize, HashSet<UsedIdent>)> =
            tree_shaking_module.get_used_export_statement().into();
        assert_debug_snapshot!(&used);
    }

    #[test]
    fn test_used_export_include_others_loop_cycle() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test.tsx"),
            r#"
// 多套一层函数，加上循环依赖
export function compile (value) {
    return console.log(parse())
}

export function parse () {
    ruleset();
}

export function ruleset () {
    compile();
}

"#,
        );
        let mut tree_shaking_module = TreeShakingModule::new(&module);
        tree_shaking_module.used_exports.add_used_export(&"compile");
        let used: Vec<(usize, HashSet<UsedIdent>)> =
            tree_shaking_module.get_used_export_statement().into();
        assert_debug_snapshot!(&used);
    }

    #[test]
    fn reexport_test() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test.tsx"),
            r#"
export * from './f1';

export { default } from './f2';

export { f } from './f3';

export { f as b } from './f4';

export { default as c } from './f5';

import { d } from './f6';
export { d };

import { g } from './f7';
import { e } from './f8';
export { e, g };
"#,
        );
        let mut tree_shaking_module = TreeShakingModule::new(&module);
        tree_shaking_module.used_exports = UsedExports::All;
        let re_exports = tree_shaking_module.get_used_re_exports();
        assert!(re_exports.is_all_re_export());
        assert_eq!(re_exports.get_all_used_statement_ids().len(), 10);
    }
}
