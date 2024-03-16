use std::collections::{HashMap, HashSet};

use bitflags::bitflags;
use serde::Serialize;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{MemberExpr, ModuleItem, Stmt, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};

use crate::module::{ImportType, ModuleId, NamedExportType, ResolveType};
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
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
    ) -> Vec<ModuleItem> {
        let mut interopee = require!(src);
        let runtime_helpers: RuntimeFlags = self.into();

        if self.contains(EsmDependantFlags::ExportAll) {
            interopee = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_export_star").into(),
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
            let ems_expose_dcl: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_wildcard").into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_dcl, ems_expose_dcl];
        }

        if runtime_helpers.contains(RuntimeFlags::DefaultInterOp) {
            let esm_expose_stmt: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_default").into(),
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
            .into_var_decl(VarDeclKind::Var, quote_ident!(stringify!($name)).into())
            .into()
    };
}

impl RuntimeFlags {
    pub fn need_op(&self) -> bool {
        self.contains(RuntimeFlags::DefaultInterOp) || self.contains(RuntimeFlags::WildcardInterOp)
    }

    pub fn interop_runtime_helpers(&self) -> (Vec<ModuleItem>, Vec<String>) {
        let mut items = vec![];
        let mut vars = vec![];

        self.iter().for_each(|flag| match flag {
            RuntimeFlags::WildcardInterOp => {
                let stmt: Stmt = dcl!(
                    _interop_require_wildcard,
                    require!("@swc/helpers/_/_interop_require_wildcard")
                );
                items.push(stmt.into());
                vars.push("_interop_require_wildcard".to_string());
            }
            RuntimeFlags::DefaultInterOp => {
                let stmt: Stmt = dcl!(
                    _interop_require_default,
                    require!("@swc/helpers/_/_interop_require_default")
                );
                items.push(stmt.into());
                vars.push("_interop_require_default".to_string());
            }
            RuntimeFlags::ExportStartInterOp => {
                let stmt: Stmt = dcl!(_export_star, require!("@swc/helpers/_/_export_star"));
                items.push(stmt.into());
                vars.push("_export_star".to_string());
            }
            _ => {}
        });

        (items, vars)
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
}

impl ConcatenateContext {
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
}
