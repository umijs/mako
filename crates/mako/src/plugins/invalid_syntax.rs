use swc_common::errors::Handler;
use swc_common::sync::Lrc;
use swc_ecma_ast::{Expr, MemberExpr, MemberProp, Module};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{Visit, VisitWith};

use crate::plugin::Plugin;

pub struct InvalidSyntaxPlugin {}

impl Plugin for InvalidSyntaxPlugin {
    fn name(&self) -> &str {
        "invalid_syntax"
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        ast: &mut swc_ecma_ast::Module,
        _context: &std::sync::Arc<crate::compiler::Context>,
    ) -> anyhow::Result<()> {
        // 先用白名单的形式，等收集的场景多了之后再考虑通用方案
        // 1、react-loadable/lib/index.js 里有用 __webpack_modules__ 来判断 isWebpackReady
        // 2、...
        if param.path.contains("node_modules") && param.path.contains("react-loadable") {
            return Ok(());
        }
        ast.visit_with(&mut InvalidSyntaxVisitor {
            bindings: None,
            handler: param.handler,
            path: param.path,
        });
        Ok(())
    }
}

pub struct InvalidSyntaxVisitor<'a> {
    pub bindings: Option<Lrc<swc_common::collections::AHashSet<swc_ecma_ast::Id>>>,
    pub handler: &'a Handler,
    pub path: &'a str,
}

impl<'a> Visit for InvalidSyntaxVisitor<'a> {
    fn visit_module(&mut self, module: &Module) {
        self.bindings = Some(Lrc::new(collect_decls(module)));
        module.visit_children_with(self);
    }
    fn visit_member_expr(&mut self, expr: &swc_ecma_ast::MemberExpr) {
        let bindings = self.bindings.clone();
        let is_require_ensure = is_member_prop(expr, "require", "ensure", true, bindings);
        if is_require_ensure {
            self.handler
                .struct_span_err(expr.span, "require.ensure syntax is not supported yet")
                .emit();
        } else {
            expr.visit_children_with(self);
        }
    }
    fn visit_ident(&mut self, n: &swc_ecma_ast::Ident) {
        let bindings = self.bindings.clone();
        // why keep __webpack_nonce__? since styled-components is using it
        let is_webpack_prefix = n.sym.starts_with("__webpack_") && &n.sym != "__webpack_nonce__";
        let has_binding = if let Some(bindings) = bindings {
            bindings.contains(&(n.sym.clone(), n.span.ctxt))
        } else {
            false
        };
        if is_webpack_prefix && !has_binding {
            self.handler
                .struct_span_err(
                    n.span,
                    format!("{} syntax is not supported yet", n.sym).as_str(),
                )
                .emit();
        } else {
            n.visit_children_with(self);
        }
    }
}

fn is_member_prop(
    expr: &swc_ecma_ast::MemberExpr,
    obj: &str,
    prop: &str,
    check_obj_binding: bool,
    bindings: Option<Lrc<swc_common::collections::AHashSet<swc_ecma_ast::Id>>>,
) -> bool {
    if let MemberExpr {
        obj: box Expr::Ident(ident),
        prop: MemberProp::Ident(prop_ident),
        ..
    } = expr
    {
        let is_obj_match = &ident.sym == obj;
        let has_binding = if let Some(bindings) = bindings {
            bindings.contains(&(ident.sym.clone(), ident.span.ctxt))
        } else {
            false
        };
        let is_prop_match = &prop_ident.sym == prop;
        is_obj_match && (check_obj_binding && !has_binding) && is_prop_match
    } else {
        false
    }
}
