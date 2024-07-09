mod param;
mod render;
mod visitor;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use swc_core::ecma::ast::Module;
use swc_core::ecma::visit::VisitMutWith;

use self::render::VirtualContextModuleRender;
use self::visitor::RequireContextVisitor;
use crate::ast::file::{Content, JsContent};
use crate::compiler::Context;
use crate::plugin::{Plugin, PluginLoadParam, PluginTransformJsParam};

pub struct RequireContextPlugin {}

impl Plugin for RequireContextPlugin {
    fn name(&self) -> &'static str {
        "require-context"
    }

    fn load(
        &self,
        param: &PluginLoadParam,
        _context: &Arc<Context>,
    ) -> anyhow::Result<Option<Content>> {
        if param
            .file
            .path
            .to_string_lossy()
            .starts_with(VIRTUAL_REQUIRE_CONTEXT_MODULE)
        {
            let params = param
                .file
                .params
                .iter()
                .cloned()
                .collect::<HashMap<String, String>>();

            let render = VirtualContextModuleRender::try_from(params)?;

            return render.render(_context.clone()).map(|content| {
                Some(Content::Js(JsContent {
                    content,
                    is_jsx: false,
                }))
            });
        }

        Ok(None)
    }

    fn transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut Module,
        context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        let mut visitor = RequireContextVisitor {
            context: context.clone(),
            current_path: PathBuf::from(param.path),
            unresolved_mark: param.unresolved_mark,
        };

        ast.visit_mut_with(&mut visitor);

        Ok(())
    }
}

const VIRTUAL_REQUIRE_CONTEXT_MODULE: &str = "virtual:context";
