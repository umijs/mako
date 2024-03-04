use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::thiserror::Error;
use mako_core::tracing::debug;

use crate::ast_2::css_ast::CssAst;
use crate::ast_2::file::{Content, File};
use crate::ast_2::js_ast::JsAst;
use crate::compiler::Context;
use crate::module::ModuleAst;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unsupported content: {path:?}")]
    UnsupportedContent { path: String },
}

pub struct Parse {}

impl Parse {
    pub fn parse(file: &File, context: Arc<Context>) -> Result<ModuleAst> {
        mako_core::mako_profile_function!(file.path);

        // let ast = context
        //     .plugin_driver
        //     .parse(&PluginParseParam { task, content }, context)?
        //     .unwrap();

        // TODO: plugin_driver
        let ast: Option<ModuleAst> = None;
        if ast.is_some() {
            return Ok(ast.unwrap());
        }

        // js
        if let Some(Content::Js(_)) = &file.content {
            debug!("parse js: {:?}", file.path);
            let ast = JsAst::new(file, context)?;
            return Ok(ModuleAst::Script(ast));
        }

        // css
        // TODO: support css modules
        if let Some(Content::Css(_)) = &file.content {
            debug!("parse css: {:?}", file.path);
            let ast = CssAst::new(file, context)?;
            return Ok(ModuleAst::Css(ast));
        }

        Err(anyhow!(ParseError::UnsupportedContent {
            path: file.path.to_string_lossy().to_string(),
        }))
    }
}
