use swc_core::common::Mark;
use swc_core::ecma::ast::{CallExpr, ImportDecl, Lit, Str};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::{is_commonjs_require, is_dynamic_import};

pub struct VirtualCSSModules {
    pub auto_css_modules: bool,
    pub unresolved_mark: Mark,
}

fn is_css_modules_path(path: &str) -> bool {
    path.ends_with(".module.css") || path.ends_with(".module.less")
}

pub fn is_css_path(path: &str) -> bool {
    path.ends_with(".css") || path.ends_with(".less") || path.ends_with(".scss")
}

impl VisitMut for VirtualCSSModules {
    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        let is_css_modules = is_css_modules_path(&import_decl.src.value)
            || (self.auto_css_modules
                && is_css_path(&import_decl.src.value)
                && !&import_decl.specifiers.is_empty());
        if is_css_modules {
            self.replace_source(&mut import_decl.src);
        }
        import_decl.visit_mut_children_with(self);
    }

    fn visit_mut_call_expr(&mut self, expr: &mut CallExpr) {
        let commonjs_require = is_commonjs_require(expr, &self.unresolved_mark);
        let dynamic_import = is_dynamic_import(expr);

        if commonjs_require || dynamic_import {
            let args = &mut expr.args;
            if args.len() == 1
                && let Some(arg) = args.first_mut()
                && arg.spread.is_none()
                && let Some(lit) = arg.expr.as_mut_lit()
                && let Lit::Str(ref mut str) = lit
            {
                let ref_ = str.value.as_ref();
                // `require()` and `import()` do not support auto_css_modules
                let is_css_modules = is_css_modules_path(ref_);
                if is_css_modules {
                    self.replace_source(str);
                }
            }
        }

        expr.visit_mut_children_with(self);
    }
}

impl VirtualCSSModules {
    fn replace_source(&mut self, source: &mut Str) {
        let to_replace = format!("{}?asmodule", &source.value.to_string());
        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::VirtualCSSModules;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_css_modules_virtual() {
        assert_eq!(
            run(r#"import "./foo.module.css";"#, false),
            r#"import "./foo.module.css?asmodule";"#
        );
        assert_eq!(
            run(r#"import "./foo.css";"#, false),
            r#"import "./foo.css";"#
        );
        assert_eq!(
            run(r#"import("./bar.module.css");"#, false),
            r#"import("./bar.module.css?asmodule");"#
        );
        assert_eq!(
            run(r#"import("./bar.css");"#, false),
            r#"import("./bar.css");"#
        );
        assert_eq!(
            run(r#"require("./baz.module.css");"#, false),
            r#"require("./baz.module.css?asmodule");"#
        );
        assert_eq!(
            run(r#"require("./baz.css");"#, false),
            r#"require("./baz.css");"#
        );
    }

    #[test]
    fn test_css_modules_virtual_when_auto_css_modules() {
        assert_eq!(
            run(r#"import x from "./foo.css";"#, true),
            r#"import x from "./foo.css?asmodule";"#
        );
        assert_eq!(
            run(r#"import "./foo.css";"#, true),
            r#"import "./foo.css";"#
        );
        assert_eq!(
            run(r#"import("./bar.css");"#, true),
            r#"import("./bar.css");"#
        );
        assert_eq!(
            run(r#"require("./baz.css");"#, true),
            r#"require("./baz.css");"#
        );
    }

    fn run(js_code: &str, auto_css_modules: bool) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        let unresolved_mark = ast.unresolved_mark;

        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = VirtualCSSModules {
                auto_css_modules,
                unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
