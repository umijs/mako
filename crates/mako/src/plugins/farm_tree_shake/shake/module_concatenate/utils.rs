use std::cmp::max;
use std::path::Path;

use heck::ToSnakeCase;
use swc_core::ecma::ast::{Ident, Stmt, VarDecl, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, ExprFactory};

use crate::module::ModuleId;

pub fn uniq_module_prefix(module_id: &ModuleId) -> String {
    let path = Path::new(&module_id.id);
    let len = path.components().count() as i32;
    let mut skip = max(len - 3, 0);
    let mut p = path.components();
    while skip > 0 {
        p.next();
        skip -= 1;
    }

    format!(
        "__$m_{}",
        p.as_path().to_string_lossy().to_string().to_snake_case()
    )
}

// for define ast: `export { orig as exported }`
#[macro_export]
macro_rules! export_as {
    ( $orig:expr => $exported:expr ) => {
        mako_core::swc_ecma_ast::ExportSpecifier::Named(
            mako_core::swc_ecma_ast::ExportNamedSpecifier {
                span: DUMMY_SP,
                orig: mako_core::swc_ecma_ast::ModuleExportName::Ident($orig),
                exported: Some(mako_core::swc_ecma_ast::ModuleExportName::Ident(
                    $exported.clone(),
                )),
                is_type_only: false,
            },
        )
    };
}

#[macro_export]
macro_rules! export {
    ( $orig:expr ) => {
        mako_core::swc_ecma_ast::ExportSpecifier::Named(
            mako_core::swc_ecma_ast::ExportNamedSpecifier {
                span: DUMMY_SP,
                orig: mako_core::swc_ecma_ast::ModuleExportName::Ident($orig),
                exported: None,
                is_type_only: false,
            },
        )
    };
}

pub fn uniq_module_default_export_name(module_id: &ModuleId) -> String {
    format!("{}_0", uniq_module_prefix(module_id))
}

pub fn uniq_module_namespace_name(module_id: &ModuleId) -> String {
    format!("{}_ns", uniq_module_prefix(module_id))
}

pub fn declare_var_with_init_stmt(name: Ident, init: &str) -> Stmt {
    declare_var_with_init(name, init).into()
}

pub fn declare_var_with_init(name: Ident, init: &str) -> VarDecl {
    quote_ident!(init).into_var_decl(VarDeclKind::Var, name.into())
}

pub const MODULE_CONCATENATE_ERROR_STR_MODULE_NAME: &str =
    "str module name not supported in module concatenation";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_files_name() {
        let module_id = ModuleId::from("/a/very/very/deep/deep/module.js");
        let name = uniq_module_prefix(&module_id);
        assert_eq!(name, "__$m_deep_deep_module_js");
    }

    #[test]
    fn long_files_name_with_query() {
        let module_id = ModuleId::from("/a/very/very/deep/deep/module.js?aQuery");
        let name = uniq_module_prefix(&module_id);
        assert_eq!(name, "__$m_deep_deep_module_js_a_query");
    }

    #[test]
    fn just_file_name() {
        let module_id = ModuleId::from("module.js");
        let name = uniq_module_prefix(&module_id);
        assert_eq!(name, "__$m_module_js");
    }

    #[test]
    fn short_file_name() {
        let module_id = ModuleId::from("/module.js");
        let name = uniq_module_prefix(&module_id);
        assert_eq!(name, "__$m_module_js");
    }

    #[test]
    fn short_file_name_with_query() {
        let module_id = ModuleId::from("/module.js?asmodules");
        let name = uniq_module_prefix(&module_id);
        assert_eq!(name, "__$m_module_js_asmodules");
    }
}
