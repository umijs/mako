use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::ast::build_js_ast;
use crate::compiler::Context;
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
}
