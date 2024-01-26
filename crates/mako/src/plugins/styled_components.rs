use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::styled_components::{styled_components, Config};
use mako_core::swc_common::FileName;
use mako_core::swc_ecma_ast::Module;
use mako_core::swc_ecma_visit::VisitMutWith;

use crate::compiler::Context;
use crate::plugin::{Plugin, PluginTransformJsParam};

pub struct StyledComponentsPlugin {}

impl Plugin for StyledComponentsPlugin {
    fn name(&self) -> &str {
        "styled_components"
    }

    fn transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut Module,
        context: &Arc<Context>,
    ) -> Result<()> {
        if context.config.styled_components.is_some() {
            let raw_config = context.config.styled_components.as_ref().unwrap();
            let pos = context.meta.script.cm.lookup_char_pos(ast.span.lo);
            let hash = pos.file.src_hash;
            let mut styled_visitor = styled_components(
                FileName::Real(param.path.into()),
                hash,
                Config {
                    display_name: raw_config.display_name,
                    ssr: raw_config.ssr,
                    file_name: raw_config.file_name,
                    meaningless_file_names: raw_config.meaningless_file_names.clone(),
                    namespace: raw_config.namespace.clone(),
                    transpile_template_literals: raw_config.transpile_template_literals,
                    minify: raw_config.minify,
                    pure: raw_config.pure,
                    css_prop: raw_config.css_prop,
                    top_level_import_paths: raw_config
                        .top_level_import_paths
                        .clone()
                        .into_iter()
                        .map(|s| s.into())
                        .collect(),
                },
            );
            ast.visit_mut_with(&mut styled_visitor);
        }

        Ok(())
    }
}
