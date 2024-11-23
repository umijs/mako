use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mako::ast::file::{Content, JsContent};
use mako::compiler::Context;
use mako::plugin::{Plugin, PluginGenerateEndParams, PluginLoadParam, PluginResolveIdParams};
use mako::resolve::{ExternalResource, Resolution, ResolvedResource, ResolverResource};

use crate::js_hook::{
    LoadResult, ResolveIdParams, ResolveIdResult, TransformResult, TsFnHooks, WatchChangesParams,
    WriteFile,
};

fn content_from_result(result: TransformResult) -> Result<Content> {
    match result.content_type.as_str() {
        "js" | "ts" => Ok(Content::Js(JsContent {
            content: result.content,
            is_jsx: false,
        })),
        "jsx" | "tsx" => Ok(Content::Js(JsContent {
            content: result.content,
            is_jsx: true,
        })),
        "css" => Ok(Content::Css(result.content)),
        _ => Err(anyhow!("Unsupported content type: {}", result.content_type)),
    }
}

pub struct JsPlugin {
    pub hooks: TsFnHooks,
    pub name: Option<String>,
    pub enforce: Option<String>,
}

impl Plugin for JsPlugin {
    fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("js_plugin")
    }

    fn enforce(&self) -> Option<&str> {
        self.enforce.as_deref()
    }

    fn build_start(&self, _context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.build_start {
            hook.call(())?
        }
        Ok(())
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.load {
            if self.hooks.load_include.is_some()
                && self
                    .hooks
                    .load_include
                    .as_ref()
                    .unwrap()
                    .call(param.file.path.to_string_lossy().to_string())?
                    == Some(false)
            {
                return Ok(None);
            }
            let x: Option<LoadResult> = hook.call(param.file.path.to_string_lossy().to_string())?;
            if let Some(x) = x {
                return content_from_result(TransformResult {
                    content: x.content,
                    content_type: x.content_type,
                })
                .map(Some);
            }
        }
        Ok(None)
    }

    fn resolve_id(
        &self,
        source: &str,
        importer: &str,
        params: &PluginResolveIdParams,
        _context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        if let Some(hook) = &self.hooks.resolve_id {
            let x: Option<ResolveIdResult> = hook.call((
                source.to_string(),
                importer.to_string(),
                ResolveIdParams {
                    is_entry: params.is_entry,
                },
            ))?;
            if let Some(x) = x {
                if let Some(true) = x.external {
                    return Ok(Some(ResolverResource::External(ExternalResource {
                        source: source.to_string(),
                        external: source.to_string(),
                        script: None,
                    })));
                }
                return Ok(Some(ResolverResource::Resolved(ResolvedResource(
                    Resolution {
                        path: PathBuf::from(x.id),
                        query: None,
                        fragment: None,
                        package_json: None,
                    },
                ))));
            }
        }
        Ok(None)
    }

    fn generate_end(&self, param: &PluginGenerateEndParams, _context: &Arc<Context>) -> Result<()> {
        // keep generate_end for compatibility
        // since build_end does not have none error params in unplugin's api spec
        if let Some(hook) = &self.hooks.generate_end {
            hook.call(serde_json::to_value(param)?)?
        }
        if let Some(hook) = &self.hooks.build_end {
            hook.call(())?
        }
        Ok(())
    }

    fn watch_changes(&self, id: &str, event: &str, _context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.watch_changes {
            hook.call((
                id.to_string(),
                WatchChangesParams {
                    event: event.to_string(),
                },
            ))?
        }
        Ok(())
    }

    fn write_bundle(&self, _context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.write_bundle {
            hook.call(())?
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

    fn load_transform(
        &self,
        content: &mut Content,
        path: &str,
        _is_entry: bool,
        _context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.transform_include {
            if hook.call(path.to_string())? == Some(false) {
                return Ok(None);
            }
        }

        if let Some(hook) = &self.hooks.transform {
            let content_str = match content {
                Content::Js(js_content) => js_content.content.clone(),
                Content::Css(css_content) => css_content.clone(),
                _ => return Ok(None),
            };

            let result: Option<TransformResult> = hook.call((content_str, path.to_string()))?;

            if let Some(result) = result {
                return content_from_result(result).map(Some);
            }
        }
        Ok(None)
    }
}
