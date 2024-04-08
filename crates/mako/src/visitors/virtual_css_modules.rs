use mako_core::swc_ecma_ast::{ImportDecl, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct VirtualCSSModules {
    pub auto_css_modules: bool,
}

fn is_css_modules_path(path: &str) -> bool {
    path.ends_with(".module.css") || path.ends_with(".module.less")
}

pub fn is_css_path(path: &str) -> bool {
    path.ends_with(".css") || path.ends_with(".less")
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
    use mako_core::swc_ecma_visit::VisitMutWith;
    use swc_core::common::GLOBALS;

    use super::VirtualCSSModules;
    use crate::ast_2::tests::TestUtils;

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
    }

    fn run(js_code: &str, auto_css_modules: bool) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code.to_string());
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = VirtualCSSModules { auto_css_modules };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
