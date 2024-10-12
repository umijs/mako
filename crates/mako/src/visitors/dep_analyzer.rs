use std::sync::Arc;

use regex::Regex;
use swc_core::common::comments::Comments;
use swc_core::common::{BytePos, Mark, Span, Spanned};
use swc_core::ecma::ast::{CallExpr, Expr, Lit, ModuleDecl, NewExpr, Str};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::ast::utils;
use crate::compiler::Context;
use crate::module::{Dependency, ImportOptions, ResolveType};
use crate::utils::create_cached_regex;

pub struct DepAnalyzer {
    pub dependencies: Vec<Dependency>,
    order: usize,
    unresolved_mark: Mark,
    context: Arc<Context>,
}

impl DepAnalyzer {
    pub fn new(unresolved_mark: Mark, context: Arc<Context>) -> Self {
        Self {
            dependencies: vec![],
            order: 1,
            unresolved_mark,
            context,
        }
    }
    fn add_dependency(&mut self, source: String, resolve_type: ResolveType, span: Option<Span>) {
        self.dependencies.push(Dependency {
            source,
            resolve_as: None,
            order: self.order,
            resolve_type,
            span,
        });
        self.order += 1;
    }

    fn analyze_import_options(&self, magic_comments_pos: BytePos) -> ImportOptions {
        let comments_texts = self
            .context
            .meta
            .script
            .origin_comments
            .read()
            .unwrap()
            .get_swc_comments()
            .get_leading(magic_comments_pos)
            .map_or(Vec::new(), |cms| {
                cms.iter().map(|c| c.text.to_string()).collect()
            });

        let chunk_name = comments_texts.iter().find_map(|t| {
            get_magic_comment_chunk_name_regex()
                .captures(t.trim())
                .and_then(|matched| matched.get(2).map(|m| m.as_str().to_string()))
        });

        ImportOptions { chunk_name }
    }
}

impl Visit for DepAnalyzer {
    fn visit_module_decl(&mut self, decl: &ModuleDecl) {
        match decl {
            // e.g.
            // import { a, b, c } from './module';
            ModuleDecl::Import(import) => {
                if import.type_only {
                    return;
                }
                let src = import.src.value.to_string();
                self.add_dependency(
                    src,
                    ResolveType::Import(import.into()),
                    Some(import.src.span),
                );
            }
            // e.g.
            // export { a, b, c } from './module';
            ModuleDecl::ExportNamed(export) => {
                if let Some(src) = &export.src {
                    self.add_dependency(
                        src.value.to_string(),
                        ResolveType::ExportNamed(export.into()),
                        Some(src.span),
                    );
                }
            }
            // e.g.
            // export * from './module';
            ModuleDecl::ExportAll(export) => {
                let src = export.src.value.to_string();
                self.add_dependency(src, ResolveType::ExportAll, Some(export.src.span));
            }
            _ => {}
        }
        // why visit_children_with(self)?
        // because the child node may contain require or import()
        // e.g. export function a() { require('b') }
        decl.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, expr: &CallExpr) {
        // e.g.
        // require('a')
        if utils::is_commonjs_require(expr, &self.unresolved_mark) {
            if let Some(src) = utils::get_first_str_arg(expr) {
                self.add_dependency(src, ResolveType::Require, Some(expr.span));
                return;
            }
        }
        // e.g.
        // import('a')
        else if utils::is_dynamic_import(expr) {
            if let Some(src) = utils::get_first_str_arg(expr) {
                let maybe_magic_comments_pos = {
                    if !expr.args.is_empty() {
                        match &expr.args[0].expr {
                            box Expr::Lit(s) => Some(s.span().lo),
                            _ => None,
                        }
                    } else {
                        None
                    }
                };

                let import_options = if self.context.config.experimental.magic_comment_chunk_name {
                    maybe_magic_comments_pos
                        .map_or(ImportOptions::default(), |magic_comments_pos| {
                            self.analyze_import_options(magic_comments_pos)
                        })
                } else {
                    ImportOptions::default()
                };

                self.add_dependency(
                    src,
                    ResolveType::DynamicImport(import_options),
                    Some(expr.span),
                );
                return;
            }
        }
        expr.visit_children_with(self);
    }

    fn visit_new_expr(&mut self, expr: &NewExpr) {
        // Web workers
        // e.g.
        // new Worker(new URL('a', import.meta.url));
        if let Some(str) = resolve_web_worker(expr, self.unresolved_mark) {
            let maybe_magic_comments_pos = expr.args.as_ref().and_then(|args| {
                if !args.is_empty() {
                    match &args[0].expr {
                        box Expr::New(s) => Some(s.span().lo),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            let import_options = if self.context.config.experimental.magic_comment_chunk_name {
                maybe_magic_comments_pos.map_or(ImportOptions::default(), |magic_comments_pos| {
                    self.analyze_import_options(magic_comments_pos)
                })
            } else {
                ImportOptions::default()
            };
            self.add_dependency(
                str.value.to_string(),
                ResolveType::Worker(import_options),
                Some(str.span),
            );
        }
        expr.visit_children_with(self);
    }
}

// get the value of url when the following conditions are met
// notice: only add dependency when the second argument is import.meta.url
// e.g.
// new Worker(new URL('a', import.meta.url));
fn resolve_web_worker(expr: &NewExpr, unresolved_mark: Mark) -> Option<&Str> {
    if !expr.args.as_ref().is_some_and(|args| !args.is_empty()) || !expr.callee.is_ident() {
        return None;
    }

    if let box Expr::Ident(ident) = &expr.callee {
        #[allow(clippy::needless_borrow)]
        if utils::is_ident_undefined(&ident, "Worker", &unresolved_mark) {
            let args = expr.args.as_ref().unwrap();
            if let Expr::New(expr) = &*args[0].expr {
                // TODO: refactor
                // use too many not operation, not intuitive
                if !expr.args.as_ref().is_some_and(|args| !args.is_empty())
                    || !expr.callee.is_ident()
                {
                    return None;
                }

                if let box Expr::Ident(ident) = &expr.callee {
                    #[allow(clippy::needless_borrow)]
                    if utils::is_ident_undefined(&ident, "URL", &unresolved_mark) {
                        let args = expr.args.as_ref().unwrap();
                        if args
                            .get(1)
                            .is_some_and(|arg| utils::is_import_meta_url(&arg.expr))
                        {
                            if let box Expr::Lit(Lit::Str(ref str)) = &args[0].expr {
                                if !utils::is_remote_or_data(&str.value) {
                                    return Some(str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn get_magic_comment_chunk_name_regex() -> Regex {
    create_cached_regex(r#"(makoChunkName|webpackChunkName):\s*['"`](\w+)['"`]"#)
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitWith;

    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(run(r#"import 'a';"#), vec!["a"]);
        // import default
        assert_eq!(run(r#"import a from 'a';"#), vec!["a"]);
        // import named
        assert_eq!(run(r#"import { a } from 'a';"#), vec!["a"]);
        // import default and named
        assert_eq!(run(r#"import a, { b } from 'a';"#), vec!["a"]);
        // import all
        assert_eq!(run(r#"import * as a from 'a';"#), vec!["a"]);
        // export named
        assert_eq!(run(r#"export { a } from "a";"#), vec!["a"]);
        // export all
        assert_eq!(run(r#"export * from "a";"#), vec!["a"]);
    }

    #[test]
    fn test_dynamic_import() {
        assert_eq!(run(r#"import('a');"#), vec!["a"]);
    }

    #[test]
    fn test_require() {
        assert_eq!(run(r#"require('a');"#), vec!["a"]);
        assert!(run(r#"const require = 'a'; require('a');"#).is_empty());
        assert!(run(r#"require(a);"#).is_empty());
    }

    #[test]
    fn test_worker() {
        assert_eq!(
            run(r#"new Worker(new URL('a', import.meta.url));"#),
            vec!["a"]
        );
        // Worker is defined
        assert!(run(r#"const Worker = 1;new Worker(new URL('a', import.meta.url));"#).is_empty());
        // URL is defined
        assert!(run(r#"const URL = 1;new Worker(new URL('a', import.meta.url));"#).is_empty());
        // no import.meta.url
        assert!(run(r#"new Worker(new URL('a'));"#).is_empty());
        // no new URL
        assert!(run(r#"new Worker('a');"#).is_empty());
        // ignore remote
        assert!(run(r#"new Worker(new URL('https://a', import.meta.url));"#).is_empty());
    }

    #[test]
    fn test_embedded() {
        assert_eq!(run(r#"export function a() { require('b') }"#), vec!["b"]);
        assert_eq!(run(r#"export function a() { import('b') }"#), vec!["b"]);
        assert_eq!(run(r#"require(require("b"))"#), vec!["b"]);
    }

    fn run(js_code: &str) -> Vec<String> {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        let mut analyzer = super::DepAnalyzer::new(ast.unresolved_mark, test_utils.context.clone());
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            ast.ast.visit_with(&mut analyzer);
        });
        let sources = analyzer
            .dependencies
            .iter()
            .map(|dep| dep.source.clone())
            .collect();
        sources
    }
}
