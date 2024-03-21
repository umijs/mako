use swc_core::ecma::ast::{ModuleDecl, ModuleItem};
use swc_core::ecma::utils::parallel::Items;
use swc_core::ecma::visit::VisitMut;

pub(crate) struct EsmHoister {}

impl EsmHoister {
    pub fn new() -> Self {
        Self {}
    }
}

impl VisitMut for EsmHoister {
    fn visit_mut_module_items(&mut self, n: &mut Vec<ModuleItem>) {
        let mut hoisted = Vec::with_capacity(n.len());
        let mut remains = Vec::with_capacity(n.len());

        for item in n.drain(..) {
            match item {
                ModuleItem::Stmt(_) => remains.push(item),
                ModuleItem::ModuleDecl(module_decl) => match module_decl {
                    ModuleDecl::Import(_) => {
                        hoisted.push(module_decl.into());
                    }
                    _ => {
                        remains.push(module_decl.into());
                    }
                },
            }
        }
        hoisted.extend(remains);
        *n = hoisted;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swc_core::ecma::visit::VisitMutWith;

    use super::*;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;
    use crate::config::Config;

    #[test]
    fn test_hoist() {
        let context = Arc::new(Context {
            config: Config {
                devtool: None,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut ast = build_js_ast("mod.js", r#"foo(); import foo from "mod""#, &context).unwrap();

        ast.ast.visit_mut_with(&mut EsmHoister::new());

        let (code, _) = js_ast_to_code(&ast.ast, &context, "test.js").unwrap();

        assert_eq!(
            code.trim(),
            r#"import foo from "mod";
foo();"#
        )
    }
}
