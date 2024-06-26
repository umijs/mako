use std::collections::HashMap;

use swc_core::common::Mark;
use swc_core::ecma::ast::{Expr, ExprOrSpread, Lit, MemberExpr, Module};
use swc_core::ecma::utils::quote_ident;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use super::ConcatenateContext;
use crate::ast::utils::is_commonjs_require;
use crate::module::ModuleId;

pub(super) struct ExternalTransformer<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub unresolved_mark: Mark,
}

impl<'a> ExternalTransformer<'_> {
    fn src_to_export_name(&'a self, src: &str) -> Option<((String, String), ModuleId)> {
        self.src_to_module.get(src).and_then(|module_id| {
            self.concatenate_context
                .external_expose_names(module_id)
                .map(|export_names| (export_names.clone(), module_id.clone()))
        })
    }

    fn require_arg_to_module_namespace(
        &self,
        args: &Vec<ExprOrSpread>,
    ) -> Option<((String, String), ModuleId)> {
        if args.len() == 1
            && let Some(arg) = args.first()
            && arg.spread.is_none()
            && let Some(lit) = arg.expr.as_lit()
            && let Lit::Str(str) = lit
        {
            self.src_to_export_name(str.value.as_ref())
        } else {
            None
        }
    }
}

impl VisitMut for ExternalTransformer<'_> {
    fn visit_mut_expr(&mut self, n: &mut Expr) {
        match n {
            // require("ext")
            Expr::Call(call_expr) if is_commonjs_require(call_expr, &self.unresolved_mark) => {
                if let Some(((namespace, _), _)) =
                    self.require_arg_to_module_namespace(&call_expr.args)
                {
                    *n = quote_ident!(namespace.clone()).into();
                }
            }
            // require("ext").foo
            Expr::Member(MemberExpr { obj, prop, .. }) => {
                if let box Expr::Call(call_expr) = obj
                    && is_commonjs_require(call_expr, &self.unresolved_mark)
                {
                    if let Some(((namespace, _), _)) =
                        self.require_arg_to_module_namespace(&call_expr.args)
                    {
                        *obj = quote_ident!(namespace.clone()).into();
                    }

                    prop.visit_mut_with(self);
                } else {
                    n.visit_mut_children_with(self);
                }
            }
            _ => {
                n.visit_mut_children_with(self);
            }
        }
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        let contains_external = self.src_to_module.values().any(|module_id| {
            self.concatenate_context
                .external_module_namespace
                .contains_key(module_id)
        });

        if contains_external {
            n.visit_mut_children_with(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use maplit::hashmap;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::resolver;

    use super::*;
    use crate::ast::js_ast::JsAst;
    use crate::compiler::Context;

    fn transform_with_external_replace(code: &str) -> String {
        let mut context: Context = Default::default();
        context.config.devtool = None;
        let context: Arc<Context> = Arc::new(context);

        let mut ast = JsAst::build("mut.js", code, context.clone()).unwrap();

        let src_2_module: HashMap<String, ModuleId> = hashmap! {
            "external".to_string() => ModuleId::from("external"),
            "external2".to_string() => ModuleId::from("external2")
        };
        let current_external_map = hashmap! {
            ModuleId::from("external") => (
                "external_namespace_cjs".to_string(), "external_namespace".to_string()
            ),
            ModuleId::from("external2") => (
                "external_namespace_cjs2".to_string(), "external_namespace2".to_string()
            )
        };

        let mut concatenate_context = ConcatenateContext {
            external_module_namespace: current_external_map,
            ..Default::default()
        };

        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));

            let mut t = ExternalTransformer {
                src_to_module: &src_2_module,
                concatenate_context: &mut concatenate_context,
                unresolved_mark: ast.unresolved_mark,
            };

            ast.ast.visit_mut_with(&mut t);
            ast.generate(context.clone())
                .unwrap()
                .code
                .trim()
                .to_string()
        })
    }

    #[test]
    fn test_require_from_external() {
        let code = transform_with_external_replace(r#"let e = require("external");"#);

        assert_eq!(code, r#"let e = external_namespace_cjs;"#.trim());
    }

    #[test]
    // the case comes from Provider generated code: let Buffer = require("buffer").Buffer;
    fn test_require_from_external_in_member_expr() {
        let code = transform_with_external_replace(r#"let e = require("external").external;"#);

        assert_eq!(code, r#"let e = external_namespace_cjs.external;"#.trim());
    }
}
