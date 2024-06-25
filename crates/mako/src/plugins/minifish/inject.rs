use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use regex::Regex;
use swc_core::common::{Mark, Span, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ExportSpecifier, Ident, ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier,
    ImportSpecifier, ImportStarAsSpecifier, MemberExpr, ModuleDecl, ModuleItem, NamedExport, Stmt,
    VarDeclKind,
};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub(super) struct MyInjector<'a> {
    unresolved_mark: Mark,
    injects: HashMap<String, &'a Inject>,
    will_inject: IndexSet<(&'a Inject, SyntaxContext)>,
    is_cjs: bool,
}

impl<'a> MyInjector<'a> {
    pub fn new(unresolved_mark: Mark, injects: HashMap<String, &'a Inject>) -> Self {
        Self {
            unresolved_mark,
            will_inject: Default::default(),
            injects,
            is_cjs: true,
        }
    }
}

impl VisitMut for MyInjector<'_> {
    fn visit_mut_ident(&mut self, n: &mut Ident) {
        if self.injects.is_empty() {
            return;
        }

        if n.span.ctxt.outer() == self.unresolved_mark {
            let name = n.sym.to_string();

            if let Some(inject) = self.injects.remove(&name) {
                self.will_inject.insert((inject, n.span.ctxt));
            }
        }
    }

    fn visit_mut_named_export(&mut self, named_export: &mut NamedExport) {
        if named_export.src.is_some() {
            named_export.visit_mut_children_with(self);
        } else {
            for spec in named_export.specifiers.iter_mut() {
                match spec {
                    ExportSpecifier::Namespace(_) | ExportSpecifier::Default(_) => {
                        spec.visit_mut_with(self);
                    }
                    ExportSpecifier::Named(named) => {
                        // skip the exported name
                        named.orig.visit_mut_with(self);
                    }
                }
            }
        }
    }

    fn visit_mut_module(&mut self, n: &mut swc_core::ecma::ast::Module) {
        n.visit_mut_children_with(self);

        let stmts = self.will_inject.iter().map(|&(inject, ctxt)| {
            if self.is_cjs || inject.prefer_require {
                inject.clone().into_require_with(ctxt, self.unresolved_mark)
            } else {
                inject.clone().into_with(ctxt)
            }
        });

        n.body.splice(0..0, stmts);
    }

    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        let has_esm = module_items.iter().any(|item| match item {
            ModuleItem::ModuleDecl(_) => true,
            ModuleItem::Stmt(_) => false,
        });

        self.is_cjs = !has_esm;

        module_items.visit_mut_children_with(self);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Inject {
    pub from: String,
    pub name: String,
    pub named: Option<String>,
    pub namespace: Option<bool>,
    pub exclude: Option<Regex>,
    pub include: Option<Regex>,
    pub prefer_require: bool,
}

impl Eq for Inject {}

impl PartialEq for Inject {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Hash for Inject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.name.as_bytes());
    }
}

impl Inject {
    fn into_require_with(self, ctxt: SyntaxContext, unresolved_mark: Mark) -> ModuleItem {
        let name_span = Span { ctxt, ..DUMMY_SP };

        let require_source_expr = quote_ident!(DUMMY_SP.apply_mark(unresolved_mark), "require")
            .as_call(DUMMY_SP, vec![quote_str!(self.from).as_arg()]);

        let stmt: Stmt = match (&self.named, &self.namespace) {
            // import { named as x }
            (Some(named), None | Some(false)) => MemberExpr {
                span: Default::default(),
                obj: require_source_expr.into(),
                prop: quote_ident!(named.to_string()).into(),
            }
            .into_var_decl(
                VarDeclKind::Var,
                quote_ident!(name_span, self.name.clone()).into(),
            )
            .into(),
            // import * as x
            (None, Some(true)) => require_source_expr
                .into_var_decl(
                    VarDeclKind::Var,
                    quote_ident!(name_span, self.name.clone()).into(),
                )
                .into(),

            // import x from "x"
            (None, None | Some(false)) => MemberExpr {
                span: DUMMY_SP,
                obj: require_source_expr.into(),
                prop: quote_ident!("default").into(),
            }
            .into_var_decl(
                VarDeclKind::Var,
                quote_ident!(name_span, self.name.clone()).into(),
            )
            .into(),
            (Some(_), Some(true)) => {
                panic!("Cannot use both `named` and `namespaced`")
            }
        };

        stmt.into()
    }

    fn into_with(self, ctxt: SyntaxContext) -> ModuleItem {
        let name_span = Span { ctxt, ..DUMMY_SP };
        let specifier: ImportSpecifier = match (&self.named, &self.namespace) {
            // import { named as x }
            (Some(named), None | Some(false)) => ImportNamedSpecifier {
                span: DUMMY_SP,
                local: quote_ident!(name_span, self.name.clone()),
                imported: if *named == self.name {
                    None
                } else {
                    Some(quote_ident!(named.to_string()).into())
                },
                is_type_only: false,
            }
            .into(),

            // import * as x
            (None, Some(true)) => ImportStarAsSpecifier {
                span: DUMMY_SP,
                local: quote_ident!(name_span, self.name),
            }
            .into(),

            // import x
            (None, None | Some(false)) => ImportDefaultSpecifier {
                span: DUMMY_SP,
                local: quote_ident!(name_span, self.name),
            }
            .into(),

            (Some(_), Some(true)) => {
                panic!("Cannot use both `named` and `namespaced`")
            }
        };

        let decl: ModuleDecl = ImportDecl {
            span: DUMMY_SP,
            specifiers: vec![specifier],
            type_only: false,
            with: None,
            src: quote_str!(self.from).into(),
        }
        .into();

        decl.into()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use maplit::hashmap;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::resolver;

    use super::*;
    use crate::ast::file::{File, JsContent};
    use crate::ast::js_ast::JsAst;
    use crate::build::analyze_deps::AnalyzeDeps;
    use crate::compiler::{Args, Context};
    use crate::module::ModuleAst;

    fn apply_inject_to_code(injects: HashMap<String, &Inject>, code: &str) -> String {
        let mut context = Context::default();
        context.config.devtool = None;
        let context = Arc::new(context);
        let mut ast = JsAst::build("cut.js", code, context.clone()).unwrap();
        let mut injector = MyInjector::new(ast.unresolved_mark, injects);
        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            ast.ast.visit_mut_with(&mut injector);
        });
        ast.generate(context.clone()).unwrap().code
    }

    #[test]
    fn no_inject() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"let my = 1;my.call("toast");"#,
        );

        assert_eq!(
            code,
            r#"let my = 1;
my.call("toast");
"#
        );
    }

    #[test]
    fn inject_from_default() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");export { }"#,
        );

        assert_eq!(
            code,
            r#"import my from "mock-lib";
my.call("toast");
export { };
"#
        );
    }

    #[test]
    fn inject_in_cjs_from_default() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");"#,
        );

        assert_eq!(
            code,
            r#"var my = require("mock-lib").default;
my.call("toast");
"#
        );
    }

    #[test]
    fn inject_from_named() {
        let i = Inject {
            name: "my".to_string(),
            named: Some("her".to_string()),
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");export { }"#,
        );
        assert_eq!(
            code,
            r#"import { her as my } from "mock-lib";
my.call("toast");
export { };
"#
        );
    }

    #[test]
    fn inject_in_cjs_from_named() {
        let i = Inject {
            name: "my".to_string(),
            named: Some("her".to_string()),
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast")"#,
        );
        assert_eq!(
            code,
            r#"var my = require("mock-lib").her;
my.call("toast");
"#
        );
    }

    #[test]
    fn inject_from_named_same_name() {
        let i = Inject {
            name: "my".to_string(),
            named: Some("my".to_string()),
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");export { }"#,
        );

        assert_eq!(
            code,
            r#"import { my } from "mock-lib";
my.call("toast");
export { };
"#
        );
    }

    #[test]
    fn inject_in_cjs_from_named_same_name() {
        let i = Inject {
            name: "my".to_string(),
            named: Some("my".to_string()),
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: false,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");"#,
        );

        assert_eq!(
            code,
            r#"var my = require("mock-lib").my;
my.call("toast");
"#
        );
    }

    #[test]
    fn inject_from_namespace() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: Some(true),
            exclude: None,
            include: None,
            prefer_require: false,
        };
        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");export { }"#,
        );

        assert_eq!(
            code,
            r#"import * as my from "mock-lib";
my.call("toast");
export { };
"#
        );
    }

    #[test]
    fn inject_in_cjs_from_namespace() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: Some(true),
            exclude: None,
            include: None,
            prefer_require: false,
        };
        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");"#,
        );

        assert_eq!(
            code,
            r#"var my = require("mock-lib");
my.call("toast");
"#
        );
    }

    #[test]
    fn injected_require_treat_as_dep() {
        let code = r#"my.call("toast");"#;
        let context = Context {
            args: Args { watch: true },
            ..Context::default()
        };
        let context = Arc::new(context);
        let file = File::with_content(
            "cut.js".to_string(),
            crate::ast::file::Content::Js(JsContent {
                content: code.to_string(),
                ..Default::default()
            }),
            context.clone(),
        );
        let mut ast = JsAst::new(&file, context.clone()).unwrap();

        GLOBALS.set(&context.meta.script.globals, || {
            let unresolved_mark = ast.unresolved_mark;
            let top_level_mark = ast.top_level_mark;
            ast.ast
                .visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
            let injects = Inject {
                name: "my".to_string(),
                named: None,
                from: "mock-lib".to_string(),
                namespace: Some(true),
                exclude: None,
                include: None,
                prefer_require: false,
            };
            ast.ast.visit_mut_with(&mut MyInjector::new(
                ast.unresolved_mark,
                hashmap! {"my".to_string() =>&injects},
            ));
        });

        let module_ast = ModuleAst::Script(ast);
        let deps = AnalyzeDeps::analyze_deps(&module_ast, &file, context.clone()).unwrap();
        println!("deps: {:?}", deps);

        assert_eq!(deps.resolved_deps.len(), 0);
        assert_eq!(deps.missing_deps.len(), 1);
    }

    #[test]
    fn inject_prefer_require() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: true,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"my.call("toast");export { }"#,
        );

        assert_eq!(
            code,
            r#"var my = require("mock-lib").default;
my.call("toast");
export { };
"#
        );
    }

    #[test]
    fn dont_inject_named_exported() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
            include: None,
            prefer_require: true,
        };

        let code = apply_inject_to_code(
            hashmap! {
                "my".to_string() =>&i
            },
            r#"let foo=1;export {foo as my}"#,
        );

        assert_eq!(
            code,
            r#"let foo = 1;
export { foo as my };
"#
        );
    }
}
