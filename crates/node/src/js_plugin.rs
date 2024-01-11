use std::sync::{mpsc, Arc};

use crate::threadsafe_function;
use crate::tsfn::{LoadResult, ReadMessage, TsFnHooks, WriteRequest};

pub struct JsPlugin {
    pub hooks: TsFnHooks,
}
use mako::compiler::Context;
use mako::load::Content;
use mako::plugin::{Plugin, PluginGenerateEndParams, PluginLoadParam};
use mako_core::anyhow::{anyhow, Result};

impl Plugin for JsPlugin {
    fn name(&self) -> &str {
        "js_plugin"
    }

    fn build_start(&self, _context: &Arc<Context>) -> Result<Option<()>> {
        if let Some(hook) = &self.hooks.build_start {
            let (tx, rx) = mpsc::channel::<napi::Result<()>>();
            hook.call(
                ReadMessage { message: (), tx },
                threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
            );
            rx.recv()
                .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))?;
        }
        Ok(None)
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.load {
            let (tx, rx) = mpsc::channel::<napi::Result<Option<LoadResult>>>();
            hook.call(
                ReadMessage {
                    message: param.task.request.path.clone(),
                    tx,
                },
                threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
            );
            let x = rx
                .recv()
                .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))?;
            if let Some(x) = x {
                match x.content_type.as_str() {
                    "js" => return Ok(Some(Content::Js(x.content))),
                    "css" => return Ok(Some(Content::Css(x.content))),
                    _ => return Err(anyhow!("Unsupported content type: {}", x.content_type)),
                }
            }
        }
        Ok(None)
    }

    fn generate_end(
        &self,
        param: &PluginGenerateEndParams,
        _context: &Arc<Context>,
    ) -> Result<Option<()>> {
        if let Some(hook) = &self.hooks.generate_end {
            let (tx, rx) = mpsc::channel::<napi::Result<()>>();
            hook.call(
                ReadMessage {
                    message: param.clone(),
                    tx,
                },
                threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
            );
            rx.recv()
                .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))?;
        }
        Ok(None)
    }

    fn before_write_fs(&self, path: &std::path::Path, content: &[u8]) -> Result<()> {
        if let Some(hook) = &self.hooks.on_generate_file {
            let (tx, rx) = mpsc::channel::<napi::Result<()>>();
            hook.call(
                WriteRequest {
                    path: path.to_path_buf(),
                    content: content.to_vec(),
                    tx,
                },
                threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
            );
            rx.recv()
                .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))?;
        }
        Ok(())
    }
}
