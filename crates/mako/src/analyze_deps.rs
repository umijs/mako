use mako_core::anyhow::Result;
use mako_core::swc_common::collections::AHashSet;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_css_ast::{ImportHref, UrlValue};
use mako_core::swc_css_visit::VisitWith as CSSVisitWith;
use mako_core::swc_ecma_ast::{CallExpr, Callee, Expr, Id, Import, Lit, Module, ModuleDecl};
use mako_core::swc_ecma_utils::collect_decls;
use mako_core::swc_ecma_visit::{Visit, VisitWith};
use mako_core::{puffin, swc_common, swc_css_ast, swc_css_visit, swc_ecma_ast};

use crate::module::{Dependency, ModuleAst, ResolveType};

pub fn analyze_deps(ast: &ModuleAst) -> Result<Vec<Dependency>> {
    puffin::profile_function!();
    match ast {
        ModuleAst::Script(ast) => analyze_deps_js(&ast.ast),
        ModuleAst::Css(ast) => analyze_deps_css(ast),
        _ => Ok(vec![]),
    }
}

pub fn analyze_deps_js(ast: &swc_ecma_ast::Module) -> Result<Vec<Dependency>> {
    let mut visitor = DepCollectVisitor::new();
    ast.visit_with(&mut visitor);
    Ok(visitor.dependencies)
}

fn analyze_deps_css(ast: &swc_css_ast::Stylesheet) -> Result<Vec<Dependency>> {
    let mut visitor = DepCollectVisitor::new();
    ast.visit_with(&mut visitor);
    Ok(visitor.dependencies)
}

pub fn is_url_ignored(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("data:")
}

pub fn handle_css_url(url: String) -> String {
    let mut url = url;
    // @import "~foo" => "foo"
    if url.starts_with('~') {
        url = url[1..].to_string();
    }
    // @import "foo" => "./foo"
    else if !url.starts_with("./") && !url.starts_with("../") {
        url = format!("./{}", url);
    }
    url
}

struct DepCollectVisitor {
    bindings: Lrc<AHashSet<Id>>,
    dependencies: Vec<Dependency>,
    order: usize,
}

impl DepCollectVisitor {
    fn new() -> Self {
        Self {
            bindings: Default::default(),
            dependencies: vec![],
            // start with 1
            // 0 for swc helpers
            order: 1,
        }
    }
    fn bind_dependency(
        &mut self,
        source: String,
        resolve_type: ResolveType,
        span: Option<swc_common::Span>,
    ) {
        self.dependencies.push(Dependency {
            source,
            order: self.order,
            resolve_type,
            span,
        });
        self.order += 1;
    }
    fn handle_css_url(&mut self, url: String) {
        if is_url_ignored(&url) {
            return;
        }
        let url = handle_css_url(url);
        self.bind_dependency(url, ResolveType::Css, None);
    }
}

impl Visit for DepCollectVisitor {
    fn visit_module(&mut self, module: &Module) {
        self.bindings = Lrc::new(collect_decls(module));
        module.visit_children_with(self);
    }
    fn visit_module_decl(&mut self, n: &ModuleDecl) {
        match n {
            ModuleDecl::Import(import) => {
                if import.type_only {
                    return;
                }
                let src = import.src.value.to_string();
                self.bind_dependency(src, ResolveType::Import, Some(import.src.span));
            }
            ModuleDecl::ExportNamed(export) => {
                if let Some(src) = &export.src {
                    self.bind_dependency(
                        src.value.to_string(),
                        ResolveType::ExportNamed,
                        Some(src.span),
                    );
                }
            }
            ModuleDecl::ExportAll(export) => {
                let src = export.src.value.to_string();
                self.bind_dependency(src, ResolveType::ExportAll, Some(export.src.span));
            }
            _ => {}
        }
        // export function xxx() {} 里可能包含 require 或 import()
        n.visit_children_with(self);
    }
    fn visit_call_expr(&mut self, expr: &CallExpr) {
        if is_commonjs_require(expr, Some(&self.bindings)) {
            if let Some(src) = get_first_arg_str(expr) {
                self.bind_dependency(src, ResolveType::Require, Some(expr.span));
                return;
            }
        } else if is_dynamic_import(expr) {
            if let Some(src) = get_first_arg_str(expr) {
                self.bind_dependency(src, ResolveType::DynamicImport, Some(expr.span));
                return;
            }
        }
        expr.visit_children_with(self);
    }
}

impl swc_css_visit::Visit for DepCollectVisitor {
    fn visit_import_href(&mut self, n: &ImportHref) {
        match n {
            // e.g.
            // @import url(a.css)
            // @import url("a.css")
            ImportHref::Url(url) => {
                let src: Option<String> = url.value.as_ref().map(|box value| match value {
                    UrlValue::Str(str) => str.value.to_string(),
                    UrlValue::Raw(raw) => raw.value.to_string(),
                });
                if let Some(src) = src {
                    self.handle_css_url(src);
                }
            }
            // e.g.
            // @import "a.css"
            ImportHref::Str(src) => {
                let src = src.value.to_string();
                self.handle_css_url(src);
            }
        }
    }
}

pub fn is_dynamic_import(call_expr: &CallExpr) -> bool {
    matches!(&call_expr.callee, Callee::Import(Import { .. }))
}

pub fn is_commonjs_require(call_expr: &CallExpr, bindings: Option<&Lrc<AHashSet<Id>>>) -> bool {
    if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident { sym, span, .. })) = &call_expr.callee
    {
        let is_require = sym == "require";
        if !is_require {
            return false;
        }
        let has_binding = if let Some(bindings) = bindings {
            bindings.contains(&(sym.clone(), span.ctxt))
        } else {
            false
        };
        !has_binding
    } else {
        false
    }
}

pub fn get_first_arg_str(call_expr: &CallExpr) -> Option<String> {
    if let Some(arg) = call_expr.args.first() {
        if let box Expr::Lit(Lit::Str(str_)) = &arg.expr {
            return Some(str_.value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{analyze_deps_js, is_url_ignored};
    use crate::ast::build_js_ast;

    #[test]
    fn test_is_url_ignored() {
        assert!(
            is_url_ignored(&String::from("http://abc")),
            "http should be ignored"
        );
        assert!(
            is_url_ignored(&String::from("https://abc")),
            "https should be ignored"
        );
        assert!(
            is_url_ignored(&String::from("data:image")),
            "data should be ignored"
        );
        assert!(
            !is_url_ignored(&String::from("./abc")),
            "./ should not be ignored"
        );
    }

    #[test]
    fn test_analyze_deps() {
        let deps = resolve(
            r#"
import 'foo';
            "#
            .trim(),
        );
        assert_eq!(deps, "foo");
    }

    #[test]
    fn test_analyze_deps_inside_exports() {
        let deps = resolve(
            r#"
export function foo() {
    require('foo');
}
            "#
            .trim(),
        );
        assert_eq!(deps, "foo");
    }

    #[test]
    fn test_analyze_deps_ignore_type_only() {
        let deps = resolve(
            r#"
import type { x } from 'foo';
import 'bar';
            "#
            .trim(),
        );
        assert_eq!(deps, "bar");
    }

    fn resolve(code: &str) -> String {
        let ast = build_js_ast("test.ts", code, &Arc::new(Default::default())).unwrap();
        let mut deps = vec![];
        deps.extend(analyze_deps_js(&ast.ast).unwrap());
        let deps = deps
            .iter()
            .map(|dep| dep.source.as_str())
            .collect::<Vec<_>>()
            .join(",");
        deps
    }
}
