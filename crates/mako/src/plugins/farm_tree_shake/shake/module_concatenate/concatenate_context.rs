use std::collections::{BTreeMap, HashMap, HashSet};

use bitflags::bitflags;
use serde::Serialize;
use swc_core::common::{SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{Id, MemberExpr, ModuleItem, VarDeclKind};
use swc_core::ecma::utils::{collect_decls_with_ctxt, quote_ident, quote_str, ExprFactory};

use crate::ast::js_ast::JsAst;
use crate::module::{ImportType, ModuleId, NamedExportType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::shake::module_concatenate::ConcatenateConfig;
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct EsmDependantFlags: u16 {
        const Default = 1;
        const Named = 1<<2;
        const ExportAll = 1<<3;
        const Namespace = 1<<4; // import * as foo, export * as foo
    }
}
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default, Ord, PartialOrd)]
    pub struct RuntimeFlags: u16 {
        const DefaultInterOp  =1;
        const WildcardInterOp = 1<<2;
        const ExportStartInterOp = 1<<3;
    }
}

macro_rules! require {
    ($src: expr) => {
        quote_ident!("require").as_call(DUMMY_SP, vec![quote_str!(DUMMY_SP, $src).as_arg()])
    };
}

impl EsmDependantFlags {
    pub fn inject_external_export_decl(
        &self,
        src: &str,
        names: &(String, String),
        interop_ident_map: &BTreeMap<RuntimeFlags, String>,
    ) -> Vec<ModuleItem> {
        let mut interopee = require!(src);
        let runtime_helpers: RuntimeFlags = self.into();

        if self.contains(EsmDependantFlags::ExportAll) {
            let ident = interop_ident_map
                .get(&RuntimeFlags::ExportStartInterOp)
                .unwrap()
                .clone();

            interopee = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(
                DUMMY_SP,
                vec![interopee.as_arg(), quote_ident!("exports").as_arg()],
            );
        }

        let cjs_expose_dcl: ModuleItem = interopee
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.0.clone()).into())
            .into();

        if runtime_helpers.contains(RuntimeFlags::WildcardInterOp) {
            let ident = interop_ident_map
                .get(&RuntimeFlags::WildcardInterOp)
                .unwrap()
                .clone();

            let ems_expose_dcl: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_dcl, ems_expose_dcl];
        }

        if runtime_helpers.contains(RuntimeFlags::DefaultInterOp) {
            let interop_ident = interop_ident_map
                .get(&RuntimeFlags::DefaultInterOp)
                .unwrap()
                .clone();

            let esm_expose_stmt: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(interop_ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_dcl, esm_expose_stmt];
        }

        if names.0.eq(&names.1) {
            return vec![cjs_expose_dcl];
        }

        let ems_expose_stmt: ModuleItem = quote_ident!(names.0.clone())
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

        vec![cjs_expose_dcl, ems_expose_stmt]
    }
}

macro_rules! dcl {
    ($name: expr, $init:expr ) => {
        $init
            .into_var_decl(VarDeclKind::Var, quote_ident!($name).into())
            .into()
    };
}

impl RuntimeFlags {
    pub fn need_op(&self) -> bool {
        self.contains(RuntimeFlags::DefaultInterOp) || self.contains(RuntimeFlags::WildcardInterOp)
    }

    pub fn op_ident(&self) -> String {
        match *self {
            RuntimeFlags::DefaultInterOp => "_interop_require_default".to_string(),
            RuntimeFlags::WildcardInterOp => "_interop_require_wildcard".to_string(),
            RuntimeFlags::ExportStartInterOp => "_export_star".to_string(),
            _ => {
                unreachable!();
            }
        }
    }

    pub fn dcl_with(&self, ident: &str) -> ModuleItem {
        match *self {
            RuntimeFlags::DefaultInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_interop_require_default"))
            }
            RuntimeFlags::WildcardInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_interop_require_wildcard"))
            }
            RuntimeFlags::ExportStartInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_export_star"))
            }
            _ => {
                unreachable!();
            }
        }
    }
}

impl From<&EsmDependantFlags> for RuntimeFlags {
    fn from(value: &EsmDependantFlags) -> Self {
        let mut rt_flags = RuntimeFlags::empty();
        if value.contains(EsmDependantFlags::Default) {
            rt_flags.insert(RuntimeFlags::DefaultInterOp);
        }

        if value.contains(EsmDependantFlags::Namespace)
            || value.contains(EsmDependantFlags::Named | EsmDependantFlags::Default)
        {
            rt_flags.remove(RuntimeFlags::DefaultInterOp);
            rt_flags.insert(RuntimeFlags::WildcardInterOp);
        }

        if value.contains(EsmDependantFlags::ExportAll) {
            rt_flags.insert(RuntimeFlags::ExportStartInterOp)
        }

        rt_flags
    }
}

impl From<EsmDependantFlags> for RuntimeFlags {
    fn from(value: EsmDependantFlags) -> Self {
        (&value).into()
    }
}

impl From<&ImportType> for EsmDependantFlags {
    fn from(value: &ImportType) -> Self {
        let mut interops = EsmDependantFlags::empty();
        value.iter().for_each(|x| match x {
            ImportType::Default => {
                interops.insert(EsmDependantFlags::Default);
            }
            ImportType::Namespace => {
                interops.insert(EsmDependantFlags::Namespace);
            }
            ImportType::Named => {
                interops.insert(EsmDependantFlags::Named);
            }
            _ => {}
        });
        interops
    }
}

impl From<&NamedExportType> for EsmDependantFlags {
    fn from(value: &NamedExportType) -> Self {
        let mut res = Self::empty();

        value.iter().for_each(|x| match x {
            NamedExportType::Default => {
                res.insert(EsmDependantFlags::Default);
            }
            NamedExportType::Named => {
                res.insert(EsmDependantFlags::Named);
            }
            NamedExportType::Namespace => {
                res.insert(EsmDependantFlags::Namespace);
            }
            _ => {}
        });
        res
    }
}

impl From<&ResolveType> for EsmDependantFlags {
    fn from(value: &ResolveType) -> Self {
        match value {
            ResolveType::Import(import_type) => import_type.into(),
            ResolveType::ExportNamed(named_export_type) => named_export_type.into(),
            ResolveType::ExportAll => EsmDependantFlags::ExportAll,
            ResolveType::Require => EsmDependantFlags::empty(),
            ResolveType::DynamicImport => EsmDependantFlags::empty(),
            ResolveType::Css => EsmDependantFlags::empty(),
            ResolveType::Worker => EsmDependantFlags::empty(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ConcatenateContext {
    pub modules_in_scope: HashMap<ModuleId, HashMap<String, String>>,
    pub top_level_vars: HashSet<String>,
    pub external_module_namespace: HashMap<ModuleId, (String, String)>,
    pub interop_idents: BTreeMap<RuntimeFlags, String>,
    pub interop_module_items: Vec<ModuleItem>,
}

impl ConcatenateContext {
    pub fn new(config: &ConcatenateConfig, module_graph: &ModuleGraph) -> Self {
        let root_module = module_graph.get_module(&config.root).unwrap();
        let ast = &mut root_module.as_script().unwrap();
        let top_level_vars = ConcatenateContext::top_level_vars(ast);

        let mut context = Self {
            top_level_vars,
            ..Default::default()
        };
        context.setup_runtime_interops(config.merged_runtime_flags());

        context
    }

    fn top_level_vars(ast: &JsAst) -> HashSet<String> {
        let mut top_level_vars = HashSet::new();
        top_level_vars.extend(
            collect_decls_with_ctxt(
                &ast.ast,
                SyntaxContext::empty().apply_mark(ast.top_level_mark),
            )
            .iter()
            .map(|id: &Id| id.0.to_string()),
        );
        top_level_vars
    }

    pub fn add_external_names(&mut self, external_id: &ModuleId, names: (String, String)) {
        self.external_module_namespace
            .insert(external_id.clone(), names);
    }
    pub fn request_safe_var_name(&mut self, base_name: &str) -> String {
        let name = self.get_safe_var_name(base_name);
        self.add_top_level_var(&name);
        name
    }

    pub fn get_safe_var_name(&self, base_name: &str) -> String {
        let mut name = base_name.to_string();

        let mut post_fix = 0;
        while self.top_level_vars.contains(&name) {
            post_fix += 1;
            name = format!("{}_{}", base_name, post_fix);
        }
        name
    }

    pub fn negotiate_safe_var_name(
        &self,
        occupied_names: &HashSet<String>,
        base_name: &str,
    ) -> String {
        let mut name = base_name.to_string();

        let mut post_fix = 0;
        while self.top_level_vars.contains(&name) || occupied_names.contains(&name) {
            post_fix += 1;
            name = format!("{}_{}", base_name, post_fix);
        }

        name
    }

    pub fn external_expose_names(&self, module_id: &ModuleId) -> Option<&(String, String)> {
        self.external_module_namespace.get(module_id)
    }

    fn add_top_level_var(&mut self, var_name: &str) -> bool {
        self.top_level_vars.insert(var_name.to_string())
    }

    fn setup_runtime_interops(&mut self, runtime_flags: RuntimeFlags) {
        for op in runtime_flags.iter() {
            let ident = self.request_safe_var_name(&op.op_ident());
            self.interop_module_items.push(op.dcl_with(&ident));
            self.interop_idents.insert(op, ident);
        }
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashmap;

    use super::*;

    #[test]
    fn test_root_top_var_conflict_with_interop() {
        let mut context: ConcatenateContext = Default::default();
        context
            .top_level_vars
            .insert("_interop_require_default".to_string());

        context.setup_runtime_interops(RuntimeFlags::DefaultInterOp);

        assert_eq!(
            context
                .interop_idents
                .into_iter()
                .collect::<HashMap<RuntimeFlags, String>>(),
            hashmap! {
                RuntimeFlags::DefaultInterOp => "_interop_require_default_1".to_string()
            }
        )
    }
}
