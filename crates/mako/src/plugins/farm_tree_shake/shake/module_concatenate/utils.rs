use std::sync::Arc;

use heck::ToSnakeCase;
use swc_core::ecma::ast::{Ident, Stmt, VarDecl, VarDeclKind};
use swc_core::ecma::utils::{quote_ident, ExprFactory};

use crate::compiler::Context;
use crate::module::ModuleId;

pub fn uniq_module_prefix(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!(
        "__mako_{}",
        module_id
            .generate(context)
            .replace("..", "u")
            .to_snake_case()
    )
}

pub fn declare_var_with_init_stmt(name: Ident, init: &str) -> Stmt {
    declare_var_with_init(name, init).into()
}

pub fn declare_var_with_init(name: Ident, init: &str) -> VarDecl {
    quote_ident!(init).into_var_decl(VarDeclKind::Var, name.into())
}

pub const MODULE_CONCATENATE_ERROR: &str =
    "module Concatenation encountered a unknown problem; please report this";
pub const MODULE_CONCATENATE_ERROR_STR_MODULE_NAME: &str =
    "str module name not supported in module concatenation";

#[cfg(test)]
mod tests {
    use super::*;

    fn default_context() -> Arc<Context> {
        let context = Context {
            root: "/my/root/path".into(),
            ..Context::default()
        };
        Arc::new(context)
    }

    #[test]
    fn root_file_name() {
        let context = default_context();

        let module_id = ModuleId::from("/my/root/path/module.js");
        let name = uniq_module_prefix(&module_id, &context);
        assert_eq!(name, "__mako_module_js");
    }

    #[test]
    fn nested_file_name() {
        let context = default_context();

        let module_id = ModuleId::from("/my/root/path/nested/module.js");
        let name = uniq_module_prefix(&module_id, &context);
        assert_eq!(name, "__mako_nested_module_js");
    }

    #[test]
    fn out_of_root_file_name() {
        let context = default_context();

        let module_id = ModuleId::from("/my/out/of/module.js");
        let name = uniq_module_prefix(&module_id, &context);
        assert_eq!(name, "__mako_u_u_out_of_module_js");
    }
}
