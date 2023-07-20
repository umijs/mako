use std::sync::Arc;

use anyhow::Result;

use crate::ast::build_js_ast;
use crate::compiler::Context;
use crate::config::Mode;
use crate::load::{read_content, Content};
use crate::module::ModuleAst;
use crate::plugin::{Plugin, PluginLoadParam, PluginParseParam};

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
            let mut content = read_content(param.path.as_str())?;
            // TODO: use array entry instead
            if param.is_entry && context.config.hmr && context.config.mode == Mode::Development {
                let port = &context.config.hmr_port.to_string();
                let host = &context.config.hmr_host.to_string();
                let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
                content = format!(
                    "{}\n{}\n",
                    content,
                    include_str!("../runtime/runtime_hmr_entry.js")
                )
                .replace("__PORT__", port)
                .replace("__HOST__", host);
            }
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
}
