use std::sync::Arc;

use heck::ToSnakeCase;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_context() -> Arc<Context> {
        let mut context = Context::default();
        context.root = "/my/root/path".into();
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
