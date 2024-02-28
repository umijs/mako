use std::collections::{HashMap, HashSet};

use bitflags::bitflags;
use serde::Serialize;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{MemberExpr, ModuleItem, Stmt, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};

use crate::module::{ImportType, ModuleId, NamedExportType, ResolveType};
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct Interops: u16 {
        const Default = 1;
        const Named = 1<<2;
        const Wildcard = 1<<3;
        const ExportAll = 1<<4;
    }
}

impl Interops {
    pub fn inject_var_decl_stmts(&self, src: &str, names: &(String, String)) -> Vec<Stmt> {
        let mut interopee =
            quote_ident!("require").as_call(DUMMY_SP, vec![quote_str!(DUMMY_SP, src).as_arg()]);

        if self.contains(Interops::ExportAll) {
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

        let cjs_expose_stmt: Stmt = interopee
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.0.clone()).into())
            .into();

        if self.contains(Interops::Wildcard) || self.contains(Interops::Named | Interops::Default) {
            let ems_expose_stmt: Stmt = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_wildcard").into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_stmt, ems_expose_stmt];
        }

        if self.contains(Interops::Default) {
            let esm_expose_stmt: Stmt = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_default").into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_stmt, esm_expose_stmt];
        }

        let ems_expose_stmt: Stmt = quote_ident!(names.0.clone())
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

        vec![cjs_expose_stmt, ems_expose_stmt]
    }

    pub fn inject_interop_runtime_helpers(&self) -> Vec<ModuleItem> {
        let mut res = vec![];

        let stmt: Stmt = quote_ident!("require")
            .as_call(
                DUMMY_SP,
                vec![quote_str!(DUMMY_SP, "@swc/helpers/_/_interop_require_default").as_arg()],
            )
            .into_var_decl(
                VarDeclKind::Var,
                quote_ident!("_interop_require_default").into(),
            )
            .into();
        res.push(stmt.into());

        let stmt: Stmt = quote_ident!("require")
            .as_call(
                DUMMY_SP,
                vec![quote_str!(DUMMY_SP, "@swc/helpers/_/_interop_require_wildcard").as_arg()],
            )
            .into_var_decl(
                VarDeclKind::Var,
                quote_ident!("_interop_require_wildcard").into(),
            )
            .into();
        res.push(stmt.into());

        if self.contains(Interops::ExportAll) {
            let stmt: Stmt = quote_ident!("require")
                .as_call(
                    DUMMY_SP,
                    vec![quote_str!(DUMMY_SP, "@swc/helpers/_/_export_star").as_arg()],
                )
                .into_var_decl(VarDeclKind::Var, quote_ident!("_export_star").into())
                .into();
            res.push(stmt.into());
        }

        res
    }
}

impl From<&ImportType> for Interops {
    fn from(value: &ImportType) -> Self {
        let mut interops = Interops::empty();
        value.iter().for_each(|x| match x {
            ImportType::Default => {
                interops.insert(Interops::Default);
            }
            ImportType::Namespace => {
                interops.insert(Interops::Wildcard);
            }
            ImportType::Named => {
                interops.insert(Interops::Named);
            }
            _ => {}
        });
        interops
    }
}

impl From<&NamedExportType> for Interops {
    fn from(value: &NamedExportType) -> Self {
        let mut res = Self::empty();

        value.iter().for_each(|x| match x {
            NamedExportType::Default => {
                res.insert(Interops::Default);
            }
            NamedExportType::Named => {}
            NamedExportType::Namespace => {
                res.insert(Interops::Wildcard);
            }
            _ => {}
        });
        res
    }
}

impl From<&ResolveType> for Interops {
    fn from(value: &ResolveType) -> Self {
        match value {
            ResolveType::Import(import_type) => import_type.into(),
            ResolveType::ExportNamed(named_export_type) => named_export_type.into(),
            ResolveType::ExportAll => Interops::ExportAll,
            ResolveType::Require => Interops::empty(),
            ResolveType::DynamicImport => Interops::empty(),
            ResolveType::Css => Interops::empty(),
            ResolveType::Worker => Interops::empty(),
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
        let mut name = base_name.to_string();

        let mut post_fix = 0;
        while self.top_level_vars.contains(&name) {
            post_fix += 1;
            name = format!("{}_{}", base_name, post_fix);
        }
        self.add_top_level_var(&name);

        name
    }
    pub fn external_expose_names(&self, module_id: &ModuleId) -> Option<&(String, String)> {
        self.external_module_namespace.get(module_id)
    }

    fn add_top_level_var(&mut self, var_name: &str) -> bool {
        self.top_level_vars.insert(var_name.to_string())
    }
}
