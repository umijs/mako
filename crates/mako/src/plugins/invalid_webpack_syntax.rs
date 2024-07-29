use anyhow;
use swc_core::common::errors::Handler;
use swc_core::common::Mark;
use swc_core::ecma::ast::{Expr, Ident, MemberExpr, MemberProp, Module};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::plugin::Plugin;

pub struct InvalidWebpackSyntaxPlugin {}

impl Plugin for InvalidWebpackSyntaxPlugin {
    fn name(&self) -> &str {
        "invalid_webpack_syntax"
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        ast: &mut Module,
        context: &std::sync::Arc<crate::compiler::Context>,
    ) -> anyhow::Result<()> {
        // 先用白名单的形式，等收集的场景多了之后再考虑通用方案
        // 1、react-loadable/lib/index.js 里有用 __webpack_modules__ 来判断 isWebpackReady
        // 2、react-server-dom-webpack contains __webpack_require__
        // 3、...
        let mut pkgs = vec![
            "react-loadable".to_string(),
            "react-server-dom-webpack".to_string(),
        ];
        pkgs.extend(context.config.experimental.webpack_syntax_validate.clone());
        // TODO: 这里的判断并不严谨，只是简单判断了路径是否包含 pkg
        // 由于要考虑 monorepo 的场景，不能直接通过 contains('node_modules') 来判断是否为三方包
        if pkgs.iter().any(|pkg| param.path.contains(pkg)) {
            return Ok(());
        }
        ast.visit_with(&mut InvalidSyntaxVisitor {
            unresolved_mark: param.unresolved_mark,
            handler: param.handler,
        });
        Ok(())
    }
}

pub struct InvalidSyntaxVisitor<'a> {
    unresolved_mark: Mark,
    pub handler: &'a Handler,
}

impl<'a> Visit for InvalidSyntaxVisitor<'a> {
    fn visit_member_expr(&mut self, expr: &MemberExpr) {
        let is_require_ensure =
            is_member_prop(expr, "require", "ensure", true, self.unresolved_mark);
        if is_require_ensure {
            self.handler
                .struct_span_err(expr.span, "require.ensure syntax is not supported yet")
                .emit();
        } else {
            expr.visit_children_with(self);
        }
    }
    fn visit_ident(&mut self, n: &Ident) {
        // why keep __webpack_nonce__? since styled-components is using it
        let is_webpack_prefix = n.sym.starts_with("__webpack_")
            && &n.sym != "__webpack_nonce__"
            && &n.sym != "__webpack_public_path__";
        let has_binding = n.span.ctxt.outer() != self.unresolved_mark;
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
    expr: &MemberExpr,
    obj: &str,
    prop: &str,
    check_obj_binding: bool,
    unresolved_mark: Mark,
) -> bool {
    if let MemberExpr {
        obj: box Expr::Ident(ident),
        prop: MemberProp::Ident(prop_ident),
        ..
    } = expr
    {
        let is_obj_match = ident.sym == obj;
        let has_binding = ident.span.ctxt.outer() != unresolved_mark;
        let is_prop_match = prop_ident.sym == prop;
        is_obj_match && (check_obj_binding && !has_binding) && is_prop_match
    } else {
        false
    }
}
