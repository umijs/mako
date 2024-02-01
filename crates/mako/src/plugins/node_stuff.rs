use std::path::{Path, PathBuf};

use mako_core::anyhow::{Ok, Result};
use mako_core::pathdiff::diff_paths;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_ecma_ast::{Expr, Lit, Module, Str};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use serde_json::Value;
use swc_core::common::{Mark, DUMMY_SP};

use crate::compiler::Args;
use crate::config::{Config, Platform};
use crate::plugin::Plugin;

pub struct NodeStuffPlugin {}

impl Plugin for NodeStuffPlugin {
    fn name(&self) -> &str {
        "node_stuff"
    }

    fn modify_config(&self, config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        if matches!(config.platform, Platform::Browser) {
            config
                .define
                .insert("__dirname".into(), Value::String("'/'".into()));
            config
                .define
                .insert("__filename".into(), Value::String("'/index.js'".into()));
        }

        Ok(())
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        ast: &mut Module,
        context: &Lrc<crate::compiler::Context>,
    ) -> Result<()> {
        if matches!(context.config.platform, Platform::Node) {
            let current_path = param.path;

            ast.visit_mut_with(&mut NodeStuff {
                unresolved_mark: param.unresolved_mark,
                current_path: &current_path.into(),
                root: &context.root,
            });
        }

        Ok(())
    }
}

struct NodeStuff<'a> {
    unresolved_mark: Mark,
    current_path: &'a PathBuf,
    root: &'a PathBuf,
}

impl VisitMut for NodeStuff<'_> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(ident) = expr
            && ident.span.ctxt.outer() == self.unresolved_mark
        {
            let is_filename = ident.sym.to_string() == "__filename";
            let is_dirname = ident.sym.to_string() == "__dirname";
            if is_filename || is_dirname {
                let path = diff_paths(self.current_path, self.root).unwrap_or("".into());
                let value = if is_filename {
                    path
                } else {
                    path.parent().unwrap_or(&PathBuf::from("")).into()
                };

                *expr = Expr::Lit(Lit::Str(Str {
                    span: DUMMY_SP,
                    value: value.to_string_lossy().into(),
                    raw: None,
                }));
            }
        }

        expr.visit_mut_children_with(self);
    }
}
