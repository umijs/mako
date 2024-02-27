use std::collections::{HashMap, HashSet};

use bitflags::bitflags;
use serde::Serialize;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{Expr, MemberExpr, ModuleItem, Stmt, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};

use crate::module::{ImportType, ModuleId, NamedExportType, ResolveType};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct Interops: u16 {
        const Default = 1;
        const Wildcard = 1<<2;
        const ExportAll = 1<<3;
    }
}

impl Interops {
    pub fn require_expr(&self, src: &str) -> Expr {
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

        if self.contains(Interops::Wildcard) {
            return MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_wildcard").into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![interopee.as_arg()]);
        }

        if self.contains(Interops::Default) {
            return MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!("_interop_require_default").into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![interopee.as_arg()]);
        }

        interopee
    }

    pub fn inject_interop_items(&self) -> Vec<ModuleItem> {
        let mut res = vec![];

        if self.contains(Interops::Default) {
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
        }

        if self.contains(Interops::Wildcard) {
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
        }

        if self.contains(Interops::ExportAll) {
            // do nothing here
            // export * will be transformed to all named exports
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
            ImportType::Named => {}
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
}
