use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::collections::AHashSet;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{self};
use mako_core::swc_ecma_ast::{
    self, CallExpr, Callee, Expr, Id, Import, Lit, Module, ModuleDecl, NewExpr,
};
use mako_core::swc_ecma_utils::collect_decls;
use mako_core::swc_ecma_visit::{Visit, VisitWith};

use crate::ast::build_js_ast;
use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::module::{Dependency, ModuleAst, ResolveType};
use crate::plugin::{Plugin, PluginDepAnalyzeParam, PluginLoadParam, PluginParseParam};

pub struct JavaScriptPlugin {}

impl Plugin for JavaScriptPlugin {
    fn name(&self) -> &str {
        "javascript"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(
            param.ext_name.as_str(),
            "js" | "jsx" | "ts" | "tsx" | "cjs" | "mjs"
        ) {
            if param.is_entry && param.request.has_query("hmr") {
                let port = &context.config.hmr_port.to_string();
                let host = &context.config.hmr_host.to_string();
                let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
                let content = format!("module.exports = require(\"{}\");", param.path.as_str());
                let content = format!(
                    "{}\n{}\n",
                    include_str!("../runtime/runtime_hmr_entry.js"),
                    content,
                )
                .replace("__PORT__", port)
                .replace("__HOST__", host);
                return Ok(Some(Content::Js(content)));
            }

            let content = read_content(param.path.as_str())?;
            return Ok(Some(Content::Js(content)));
        }
        Ok(None)
    }

    fn parse(&self, param: &PluginParseParam, context: &Arc<Context>) -> Result<Option<ModuleAst>> {
        if let Content::Js(content) = param.content {
            let ast = build_js_ast(&param.request.path, content, context)?;
            return Ok(Some(ModuleAst::Script(ast)));
        }
        Ok(None)
    }

    fn analyze_deps(&self, param: &mut PluginDepAnalyzeParam) -> Result<Option<Vec<Dependency>>> {
        if let ModuleAst::Script(script) = param.ast {
            let mut visitor = DepCollectVisitor::new();
            script.ast.visit_with(&mut visitor);
            Ok(Some(visitor.dependencies))
        } else {
            Ok(None)
        }
    }
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

    // Web Workers: new Worker()
    fn visit_new_expr(&mut self, new_expr: &NewExpr) {
        if is_web_worker(new_expr) {
            let arg = &new_expr.args.as_ref().unwrap().get(0).unwrap().expr;
            if let box Expr::Lit(Lit::Str(str)) = arg {
                self.bind_dependency(str.value.to_string(), ResolveType::Worker, None);
            }
        }

        new_expr.visit_children_with(self);
    }
}

pub fn is_web_worker(new_expr: &NewExpr) -> bool {
    if !new_expr.args.is_some_and(|args| !args.is_empty()) || !new_expr.callee.is_ident() {
        return false;
    }

    new_expr.callee.as_ident().unwrap().sym.eq("Worker")
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
