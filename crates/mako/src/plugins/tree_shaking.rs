use std::sync::Arc;

use anyhow::Result;
use swc_core::common::util::take::Take;
use swc_core::ecma::ast::{Decl, Module, ModuleItem, Stmt, VarDecl};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module_graph::ModuleGraph;
use crate::plugin::{Plugin, PluginTransformJsParam};

mod collect_explicit_prop;
mod module;
mod module_side_effects_flag;
mod remove_useless_stmts;
mod shake;
mod statement_graph;

pub struct FarmTreeShake {}

impl Plugin for FarmTreeShake {
    fn name(&self) -> &str {
        "tree-shake"
    }

    fn transform_js(
        &self,
        _param: &PluginTransformJsParam,
        ast: &mut Module,
        context: &Arc<Context>,
    ) -> Result<()> {
        if context.config._tree_shaking.is_some() {
            ast.visit_mut_with(&mut TopLevelDeclSplitter {});
        }
        Ok(())
    }

    fn optimize_module_graph(
        &self,
        module_graph: &mut ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<()> {
        shake::optimize_modules(module_graph, context)?;
        Ok(())
    }
}

struct TopLevelDeclSplitter {}

impl VisitMut for TopLevelDeclSplitter {
    fn visit_mut_module_items(&mut self, modules: &mut Vec<ModuleItem>) {
        let mut replaces = vec![];
        let mut items_added = 0;

        modules.iter_mut().enumerate().for_each(|(i, module_item)| {
            if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) = module_item {
                if var_decl.decls.len() > 1 {
                    let declarators = var_decl.decls.take();

                    let kind = var_decl.kind;

                    let items = declarators
                        .into_iter()
                        .map(|decl| {
                            let i: ModuleItem = VarDecl {
                                span: decl.span,
                                kind,
                                declare: false,
                                decls: vec![decl],
                            }
                            .into();

                            i
                        })
                        .collect::<Vec<_>>();

                    items_added += items.len() - 1;
                    replaces.push((i, items));
                }
            }
        });

        replaces.reverse();
        modules.reserve_exact(items_added);

        replaces.into_iter().for_each(|(i, items)| {
            modules.splice(i..i + 1, items);
        });
    }
}

#[cfg(test)]
mod tests {
    use swc_core::ecma::visit::VisitMutWith;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_split_multi_declarator_decl() {
        assert_eq!(
            split_top_decl(r#" var a = 1, b = 2; "#),
            r#"
var a = 1;
var b = 2;
"#
            .trim()
        )
    }

    #[test]
    fn test_single_declarator_decl() {
        assert_eq!(split_top_decl("var a = 1;"), "var a = 1;");
    }

    #[test]
    fn test_non_toplevel_decl() {
        assert_eq!(
            split_top_decl(
                r#"{
    var a = 1, b =2;
}"#
            ),
            r#"{
    var a = 1, b = 2;
}"#
        );
    }

    fn split_top_decl(code: &str) -> String {
        let mut tu = TestUtils::gen_js_ast(code);

        tu.ast
            .js_mut()
            .ast
            .visit_mut_with(&mut TopLevelDeclSplitter {});
        tu.js_ast_to_code()
    }
}
