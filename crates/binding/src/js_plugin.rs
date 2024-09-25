use std::sync::Arc;

use crate::js_hook::{LoadResult, TsFnHooks, WriteFile};

pub struct JsPlugin {
    pub hooks: TsFnHooks,
}
use anyhow::{anyhow, Result};
use mako::ast::file::{Content, JsContent};
use mako::compiler::Context;
use mako::plugin::{Plugin, PluginGenerateEndParams, PluginLoadParam};

impl Plugin for JsPlugin {
    fn name(&self) -> &str {
        "js_plugin"
    }

    fn build_start(&self, _context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.build_start {
            hook.call(())?
        }
        Ok(())
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.load {
            let x: Option<LoadResult> = hook.call(param.file.path.to_string_lossy().to_string())?;
            if let Some(x) = x {
                match x.content_type.as_str() {
                    "js" | "ts" => {
                        return Ok(Some(Content::Js(JsContent {
                            content: x.content,
                            is_jsx: false,
                        })))
                    }
                    "jsx" | "tsx" => {
                        return Ok(Some(Content::Js(JsContent {
                            content: x.content,
                            is_jsx: true,
                        })))
                    }
                    "css" => return Ok(Some(Content::Css(x.content))),
                    _ => return Err(anyhow!("Unsupported content type: {}", x.content_type)),
                }
            }
        }
        Ok(None)
    }

    fn generate_end(&self, param: &PluginGenerateEndParams, _context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.generate_end {
            hook.call(serde_json::to_value(param)?)?
        }
        Ok(())
    }

    fn before_write_fs(&self, path: &std::path::Path, content: &[u8]) -> Result<()> {
        if let Some(hook) = &self.hooks._on_generate_file {
            hook.call(WriteFile {
                path: path.to_string_lossy().to_string(),
                content: content.to_vec(),
            })?;
        }
        Ok(())
    }
}
