use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::thiserror::Error;
use mako_core::tracing::debug;

use crate::ast_2::css_ast::CssAst;
use crate::ast_2::file::{Content, File};
use crate::ast_2::js_ast::JsAst;
use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::plugin::PluginParseParam;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unsupported content: {path:?}")]
    UnsupportedContent { path: String },
}

pub struct Parse {}

impl Parse {
    pub fn parse(file: &File, context: Arc<Context>) -> Result<ModuleAst> {
        mako_core::mako_profile_function!(file.path.to_string_lossy());

        // plugin first
        let ast = context
            .plugin_driver
            .parse(&PluginParseParam { file }, &context)?;
        if let Some(ast) = ast {
            return Ok(ast);
        }

        // js
        if let Some(Content::Js(_)) = &file.content {
            debug!("parse js: {:?}", file.path);
            let ast = JsAst::new(file, context)?;
            return Ok(ModuleAst::Script(ast));
        }

        // css
        if let Some(Content::Css(_)) = &file.content {
            debug!("parse css: {:?}", file.path);
            let is_modules = file.has_param("modules");
            let is_asmodule = file.has_param("asmodule");
            let css_modules = is_modules || is_asmodule;
            let mut ast = CssAst::new(file, context.clone(), css_modules)?;
            if is_asmodule {
                let mut file = file.clone();
                file.set_content(Content::Js(CssAst::generate_css_modules_exports(
                    &file.pathname.to_string_lossy(),
                    &mut ast.ast,
                    context.config.css_modules_export_only_locales,
                )));
                let ast = JsAst::new(&file, context)?;
                return Ok(ModuleAst::Script(ast));
            } else {
                return Ok(ModuleAst::Css(ast));
            }
        }

        Err(anyhow!(ParseError::UnsupportedContent {
            path: file.path.to_string_lossy().to_string(),
        }))
    }
}
