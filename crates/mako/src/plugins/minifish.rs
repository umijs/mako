use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::regex::Regex;
use mako_core::swc_common::{Mark, Span, SyntaxContext, DUMMY_SP};
use mako_core::swc_ecma_ast::{
    Ident, ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier,
    ImportStarAsSpecifier, MemberExpr, ModuleDecl, ModuleItem, Stmt, VarDeclKind,
};
use mako_core::swc_ecma_utils::{quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use serde::Serialize;

use crate::compiler::Context;
use crate::load::Content::Assets;
use crate::load::{read_content, Asset, Content};
use crate::module::{Dependency as ModuleDependency, ModuleAst, ResolveType};
use crate::plugin::{Plugin, PluginLoadParam, PluginParseParam, PluginTransformJsParam};
use crate::plugins::bundless_compiler::to_dist_path;
use crate::stats::StatsJsonMap;

pub struct MinifishPlugin {
    pub mapping: HashMap<String, String>,
    pub meta_path: Option<PathBuf>,
    pub inject: Option<HashMap<String, Inject>>,
}

impl MinifishPlugin {}

impl Plugin for MinifishPlugin {
    fn name(&self) -> &str {
        "minifish_plugin"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "json" | "json5") {
            let root = _context.root.clone();
            let to: PathBuf = param.path.clone().into();

            let relative = to
                .strip_prefix(root)
                .unwrap_or_else(|_| panic!("{:?} not under project root", to))
                .to_str()
                .unwrap();

            return match self.mapping.get(relative) {
                Some(js_content) => Ok(Some(Content::Js(js_content.to_string()))),

                None => {
                    let content = read_content(param.path.as_str())?;

                    let asset = Asset {
                        path: param.path.clone(),
                        content,
                    };

                    Ok(Some(Assets(asset)))
                }
            };
        }
        Ok(None)
    }

    fn parse(
        &self,
        param: &PluginParseParam,
        _context: &Arc<Context>,
    ) -> Result<Option<ModuleAst>> {
        if param.request.path.ends_with(".json") {
            if let Assets(_) = param.content {
                return Ok(Some(ModuleAst::None));
            }
        }

        Ok(None)
    }

    fn transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut mako_core::swc_ecma_ast::Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        if let Some(inject) = &self.inject {
            if inject.is_empty() {
                return Ok(());
            }

            let mut matched_injects = HashMap::new();

            for (k, i) in inject {
                if let Some(exclude) = &i.exclude {
                    if !exclude.is_match(param.path) {
                        matched_injects.insert(k.clone(), i);
                    }
                } else {
                    matched_injects.insert(k.clone(), i);
                }
            }

            if matched_injects.is_empty() {
                return Ok(());
            }

            ast.visit_mut_with(&mut MyInjector::new(param.unresolved_mark, matched_injects));
        }
        Ok(())
    }

    fn before_resolve(
        &self,
        deps: &mut Vec<ModuleDependency>,
        _context: &Arc<Context>,
    ) -> Result<()> {
        let src_root = _context
            .config
            .output
            .preserve_modules_root
            .to_str()
            .ok_or_else(|| {
                anyhow!(
                    "output.preserve_modules_root {:?} is not a valid utf8 string",
                    _context.config.output.preserve_modules_root
                )
            })?;

        if src_root.is_empty() {
            return Err(anyhow!(
                "output.preserve_modules_root cannot be empty in minifish plugin"
            ));
        }

        for dep in deps.iter_mut() {
            if dep.source.starts_with('/') {
                let mut resolve_as = dep.source.clone();
                resolve_as.replace_range(0..0, src_root);
                dep.resolve_as = Some(resolve_as);
            }
        }

        Ok(())
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        if let Some(meta_path) = &self.meta_path {
            let mg = context.module_graph.read().unwrap();

            let ids = mg.get_module_ids();

            let modules: Vec<_> = ids
                .par_iter()
                .map(|id| {
                    let deps: Vec<_> = mg
                        .get_dependencies(id)
                        .iter()
                        .map(|dep| Dependency {
                            module: dep.0.id.clone(),
                            import_type: dep.1.resolve_type,
                        })
                        .collect();

                    let filename = if id.id.ends_with(".json") {
                        to_dist_path(&id.id, context).to_string_lossy().to_string()
                    } else {
                        to_dist_path(&id.id, context)
                            .with_extension("js")
                            .to_string_lossy()
                            .to_string()
                    };

                    Module {
                        filename,
                        id: id.id.clone(),
                        dependencies: deps,
                    }
                })
                .collect();

            let meta =
                serde_json::to_string_pretty(&serde_json::json!(ModuleGraphOutput { modules }))
                    .unwrap();

            std::fs::create_dir_all(meta_path.parent().unwrap()).unwrap();

            std::fs::write(meta_path, meta)
                .map_err(|e| anyhow!("write meta file({}) error: {}", meta_path.display(), e))?;
        }

        Ok(None)
    }
}

struct MyInjector<'a> {
    unresolved_mark: Mark,
    injects: HashMap<String, &'a Inject>,
    will_inject: IndexSet<(&'a Inject, SyntaxContext)>,
    is_cjs: bool,
}

impl<'a> MyInjector<'a> {
    fn new(unresolved_mark: Mark, injects: HashMap<String, &'a Inject>) -> Self {
        Self {
            unresolved_mark,
            will_inject: Default::default(),
            injects,
            is_cjs: true,
        }
    }
}

impl VisitMut for MyInjector<'_> {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        let has_esm = module_items.iter().any(|item| match item {
            ModuleItem::ModuleDecl(_) => true,
            ModuleItem::Stmt(_) => false,
        });

        self.is_cjs = !has_esm;

        module_items.visit_mut_children_with(self);
    }

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

    fn visit_mut_module(&mut self, n: &mut mako_core::swc_ecma_ast::Module) {
        n.visit_mut_children_with(self);

        self.will_inject.iter().for_each(|&(inject, ctxt)| {
            let mi = if self.is_cjs || inject.prefer_require {
                inject.clone().into_require_with(ctxt, self.unresolved_mark)
            } else {
                inject.clone().into_with(ctxt)
            };

            n.body.insert(0, mi);
        });
    }
}

#[derive(Clone, Debug)]
pub struct Inject {
    pub from: String,
    pub name: String,
    pub named: Option<String>,
    pub namespace: Option<bool>,
    pub exclude: Option<Regex>,
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

#[derive(Serialize)]
struct ModuleGraphOutput {
    modules: Vec<Module>,
}

#[derive(Serialize)]
struct Module {
    filename: String,
    id: String,
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    module: String,
    import_type: ResolveType,
}

#[cfg(test)]
mod tests {
    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_transforms::resolver;
    use maplit::hashmap;

    use super::*;
    use crate::analyze_deps::analyze_deps;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::config::DevtoolConfig;
    use crate::plugin::PluginDriver;
    use crate::plugins::javascript::JavaScriptPlugin;

    fn apply_inject_to_code(injects: HashMap<String, &Inject>, code: &str) -> String {
        let mut context = Context::default();
        context.config.devtool = DevtoolConfig::None;
        let context = Arc::new(context);

        let mut ast = build_js_ast("cut.js", code, &context).unwrap();

        let mut injector = MyInjector::new(ast.unresolved_mark, injects);

        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            ast.ast.visit_mut_with(&mut injector);
        });

        let (code, _) = js_ast_to_code(&ast.ast, &context, "x.js").unwrap();

        code
    }

    #[test]
    fn no_inject() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
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
        let injects = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: Some(true),
            exclude: None,
            prefer_require: false,
        };

        let mut context = Context {
            plugin_driver: PluginDriver::new(vec![Arc::new(JavaScriptPlugin {})]),
            ..Context::default()
        };
        context.config.devtool = DevtoolConfig::None;
        let context = Arc::new(context);

        let mut ast = build_js_ast("cut.js", code, &context).unwrap();

        let mut injector =
            MyInjector::new(ast.unresolved_mark, hashmap! {"my".to_string() =>&injects});
        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            ast.ast.visit_mut_with(&mut injector);
        });

        let module_ast = ModuleAst::Script(ast);

        let deps = analyze_deps(&module_ast, &context).unwrap();

        assert_eq!(deps.len(), 1);
    }

    #[test]
    fn inject_prefer_require() {
        let i = Inject {
            name: "my".to_string(),
            named: None,
            from: "mock-lib".to_string(),
            namespace: None,
            exclude: None,
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
}
