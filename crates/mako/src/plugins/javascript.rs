use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::{self, Mark, GLOBALS};
use mako_core::swc_ecma_ast::{
    CallExpr, Callee, Expr, Ident, Import, Lit, MemberExpr, MemberProp, MetaPropExpr, MetaPropKind,
    ModuleDecl, NewExpr, Str,
};
use mako_core::swc_ecma_visit::{Visit, VisitWith};

use super::css::is_url_ignored;
use crate::ast::build_js_ast;
use crate::compiler::Context;
use crate::config::Platform;
use crate::load::{read_content, Content};
use crate::module::{Dependency, ModuleAst, ResolveType};
use crate::plugin::{Plugin, PluginDepAnalyzeParam, PluginLoadParam, PluginParseParam};

pub struct JavaScriptPlugin {}

impl Plugin for JavaScriptPlugin {
    fn name(&self) -> &str {
        "javascript"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if param
            .task
            .is_match(vec!["js", "jsx", "ts", "tsx", "cjs", "mjs"])
        {
            if param.task.is_entry && param.task.request.has_query("hmr") {
                let port = &context.config.hmr.as_ref().unwrap().port.to_string();
                let host = &context.config.hmr.as_ref().unwrap().host.to_string();
                let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
                let mut content = format!(
                    "module.exports = require(\"{}\");",
                    param.task.request.path.as_str()
                );
                let is_browser = matches!(context.config.platform, Platform::Browser);
                if is_browser {
                    content = format!(
                        "{}\n{}\n",
                        include_str!("../runtime/runtime_hmr_entry.js"),
                        content,
                    )
                    .replace("__PORT__", port)
                    .replace("__HOST__", host);
                }
                return Ok(Some(Content::Js(content)));
            }

            let content = read_content(param.task.request.path.as_str())?;
            return Ok(Some(Content::Js(content)));
        }
        Ok(None)
    }

    fn parse(&self, param: &PluginParseParam, context: &Arc<Context>) -> Result<Option<ModuleAst>> {
        if let Content::Js(content) = param.content {
            let ast = build_js_ast(&param.task.request.path, content, context)?;
            return Ok(Some(ModuleAst::Script(ast)));
        }
        Ok(None)
    }

    fn analyze_deps(
        &self,
        param: &mut PluginDepAnalyzeParam,
        context: &Arc<Context>,
    ) -> Result<Option<Vec<Dependency>>> {
        if let ModuleAst::Script(script) = param.ast {
            let mut visitor = DepCollectVisitor::new(script.unresolved_mark);
            GLOBALS.set(&context.meta.script.globals, || {
                script.ast.visit_with(&mut visitor);
                Ok(Some(visitor.dependencies))
            })
        } else {
            Ok(None)
        }
    }
}

pub struct DepCollectVisitor {
    dependencies: Vec<Dependency>,
    order: usize,
    unresolved_mark: Mark,
}

impl DepCollectVisitor {
    pub(crate) fn new(unresolved_mark: Mark) -> Self {
        Self {
            dependencies: vec![],
            // start with 1
            // 0 for swc helpers
            order: 1,
            unresolved_mark,
        }
    }

    #[cfg(test)]
    pub(crate) fn dependencies(&self) -> &Vec<Dependency> {
        &self.dependencies
    }

    fn bind_dependency(
        &mut self,
        source: String,
        resolve_type: ResolveType,
        span: Option<swc_common::Span>,
    ) {
        self.dependencies.push(Dependency {
            source,
            resolve_as: None,
            order: self.order,
            resolve_type,
            span,
        });
        self.order += 1;
    }
}

impl Visit for DepCollectVisitor {
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
        if is_commonjs_require(expr, &self.unresolved_mark) {
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

    // Web workers
    fn visit_new_expr(&mut self, new_expr: &NewExpr) {
        if let Some(str) = resolve_web_worker(new_expr, self.unresolved_mark) {
            self.bind_dependency(str.value.to_string(), ResolveType::Worker, None);
        }

        new_expr.visit_children_with(self);
    }
}

pub fn resolve_web_worker(new_expr: &NewExpr, unresolved_mark: Mark) -> Option<&Str> {
    if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty()) || !new_expr.callee.is_ident() {
        return None;
    }

    if let box Expr::Ident(Ident { span, sym, .. }) = &new_expr.callee {
        // `Worker` must be unresolved
        if sym == "Worker" && (span.ctxt.outer() == unresolved_mark) {
            let args = new_expr.args.as_ref().unwrap();

            // new Worker(new URL(''), base);
            if let Expr::New(new_expr) = &*args[0].expr {
                if !new_expr.args.as_ref().is_some_and(|args| !args.is_empty())
                    || !new_expr.callee.is_ident()
                {
                    return None;
                }

                if let box Expr::Ident(Ident { span, sym, .. }) = &new_expr.callee {
                    if sym == "URL" && (span.ctxt.outer() == unresolved_mark) {
                        // new URL(''); 仅第一个参数为字符串字面量, 第二个参数为 import.meta.url 时添加依赖
                        let args = new_expr.args.as_ref().unwrap();

                        if args.get(1).is_none() || !is_import_meta_url(&args.get(1).unwrap().expr)
                        {
                            return None;
                        }

                        if let box Expr::Lit(Lit::Str(ref str)) = &args[0].expr {
                            if !is_url_ignored(&str.value) {
                                return Some(str);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn is_import_meta_url(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Member(MemberExpr {
            obj:
                box Expr::MetaProp(MetaPropExpr {
                    kind: MetaPropKind::ImportMeta,
                    ..
                }),
            prop:
                MemberProp::Ident(Ident {
                    sym,
                    ..
                }),
            ..
        }) if sym == "url"
    )
}

pub fn is_dynamic_import(call_expr: &CallExpr) -> bool {
    matches!(&call_expr.callee, Callee::Import(Import { .. }))
}

pub fn is_commonjs_require(call_expr: &CallExpr, unresolved_mark: &Mark) -> bool {
    if let Callee::Expr(box Expr::Ident(ident)) = &call_expr.callee {
        ident.sym == *"require" && is_native_ident(ident, unresolved_mark)
            || ident.sym == *"__mako_require__"
    } else {
        false
    }
}

pub fn is_native_ident(ident: &Ident, unresolved_mark: &Mark) -> bool {
    let outer = ident.span.ctxt.outer();

    outer == *unresolved_mark
}

pub fn get_first_arg_str(call_expr: &CallExpr) -> Option<String> {
    if let Some(arg) = call_expr.args.first() {
        if let box Expr::Lit(Lit::Str(str_)) = &arg.expr {
            return Some(str_.value.to_string());
        }
    }
    None
}
