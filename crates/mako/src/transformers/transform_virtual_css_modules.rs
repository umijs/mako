use std::sync::Arc;

use mako_core::lazy_static::lazy_static;
use mako_core::regex::Regex;
use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::{ImportDecl, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_core::ecma::ast::{CallExpr, ExprOrSpread, Lit};
use swc_core::ecma::utils::{quote_str, ExprFactory};

use crate::compiler::Context;

pub struct VirtualCSSModules<'a> {
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
}

lazy_static! {
    static ref CSS_MODULES_PATH_REGEX: Regex = Regex::new(r#"\.module\.(css|less)$"#).unwrap();
    static ref CSS_PATH_REGEX: Regex = Regex::new(r#"\.(css|less)$"#).unwrap();
}

pub fn is_css_modules_path(path: &str) -> bool {
    CSS_MODULES_PATH_REGEX.is_match(path)
}

pub fn is_css_path(path: &str) -> bool {
    CSS_PATH_REGEX.is_match(path)
}

impl VisitMut for VirtualCSSModules<'_> {
    fn visit_mut_call_expr(&mut self, n: &mut CallExpr) {
        if n.args.len() == 1 {
            if n.callee.is_import() {
                self.replace_first_arg(&mut n.args);
            } else if let Some(call_expr) = n.callee.as_expr()
                && let Some(callee_ident) = call_expr.as_ident()
                && callee_ident.span.ctxt.outer() == self.unresolved_mark
                && callee_ident.sym.as_str() == "require"
            {
                self.replace_first_arg(&mut n.args);
            }
        }

        n.visit_mut_children_with(self);
    }
    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        if is_css_modules_path(&import_decl.src.value)
            || (self.context.config.auto_css_modules
                && is_css_path(&import_decl.src.value)
                && !&import_decl.specifiers.is_empty())
        {
            self.replace_source(&mut import_decl.src);
        }
        import_decl.visit_mut_children_with(self);
    }
}

impl VirtualCSSModules<'_> {
    fn replace_first_arg(&mut self, args: &mut Vec<ExprOrSpread>) {
        if let Some(first_arg) = args.get_mut(0) {
            if let Some(lit) = first_arg.expr.as_lit()
                && let Lit::Str(import_str) = lit
            {
                let origin_import_str = import_str.value.as_str();

                if is_css_modules_path(origin_import_str) {
                    let replaced = format!("{}?asmodule", origin_import_str);
                    *first_arg = quote_str!(replaced).as_arg();
                }
            }
        }
    }

    fn replace_source(&mut self, source: &mut Str) {
        let to_replace = format!("{}?asmodule", &source.value.to_string());
        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::resolver;
    use swc_core::ecma::visit::VisitMutWith;

    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;
    use crate::transformers::transform_virtual_css_modules::VirtualCSSModules;

    #[test]
    fn test_dynamic_import_css_module() {
        let code = act_replace(
            r#"
            import('./styles.module.css').then();
            "#,
        );

        assert_eq!(
            code.trim(),
            r#"import("./styles.module.css?asmodule").then();"#
        )
    }

    #[test]
    fn test_dynamic_import_non_css_module() {
        let code = act_replace(
            r#"
            import("./styles.css").then();
            "#,
        );

        assert_eq!(code.trim(), r#"import("./styles.css").then();"#)
    }

    #[test]
    fn test_require_css_module() {
        let code = act_replace(r#"require("./style.module.css")"#);

        assert_eq!(code.trim(), r#"require("./style.module.css?asmodule");"#)
    }

    #[test]
    fn test_require_no_css_module() {
        let code = act_replace(r#"require("./style.css")"#);

        assert_eq!(code.trim(), r#"require("./style.css");"#)
    }

    fn act_replace(code: &str) -> String {
        let mut context: Context = Default::default();
        context.config.devtool = None;
        let context: Arc<Context> = Arc::new(context);

        let mut ast = build_js_ast("sut.js", code, &context).unwrap();

        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));

            ast.ast.visit_mut_with(&mut VirtualCSSModules {
                context: &context,
                unresolved_mark: ast.unresolved_mark,
            });
        });

        let (code, _) = js_ast_to_code(&ast.ast, &context, "sut.js").unwrap();
        code
    }
}
