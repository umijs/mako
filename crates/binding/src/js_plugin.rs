use std::path::PathBuf;
use std::sync::{Arc, Weak};

use anyhow::{anyhow, Result};
use mako::ast::file::{Content, File, JsContent};
use mako::compiler::Context;
use mako::module::{Dependency, ResolveType};
use mako::plugin::{Plugin, PluginGenerateEndParams, PluginLoadParam, PluginResolveIdParams};
use mako::resolve::{ExternalResource, Resolution, ResolvedResource, ResolverResource};
use napi_derive::napi;

use crate::js_hook::{
    AddDepsResult, LoadResult, ResolveIdParams, ResolveIdResult, TransformResult, TsFnHooks,
    WatchChangesParams, WriteFile,
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

#[napi]
pub struct PluginContext {
    context: Weak<Context>,
}

#[napi]
impl PluginContext {
    #[napi]
    pub fn warn(&self, msg: String) {
        println!("WARN: {}", msg)
    }
    #[napi]
    pub fn error(&self, msg: String) {
        println!("ERROR: {}", msg)
    }
    #[napi]
    pub fn emit_file(&self, origin_path: String, output_path: String) {
        let mut assets_info = {
            unsafe {
                self.context
                    .as_ptr()
                    .as_ref_unchecked()
                    .assets_info
                    .lock()
                    .unwrap()
            }
        };
        assets_info.insert(origin_path, output_path);
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

    fn build_start(&self, context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.build_start {
            hook.call(PluginContext {
                context: Arc::downgrade(context),
            })?
        }
        Ok(())
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.load {
            if self.hooks.load_include.is_some()
                && self.hooks.load_include.as_ref().unwrap().call((
                    PluginContext {
                        context: Arc::downgrade(context),
                    },
                    param.file.path.to_string_lossy().to_string(),
                ))? == Some(false)
            {
                return Ok(None);
            }
            let x: Option<LoadResult> = hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                param.file.path.to_string_lossy().to_string(),
            ))?;
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
        context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        if let Some(hook) = &self.hooks.resolve_id {
            let x: Option<ResolveIdResult> = hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
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

    fn generate_end(&self, param: &PluginGenerateEndParams, context: &Arc<Context>) -> Result<()> {
        // keep generate_end for compatibility
        // since build_end does not have none error params in unplugin's api spec
        if let Some(hook) = &self.hooks.generate_end {
            hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                serde_json::to_value(param)?,
            ))?
        }
        if let Some(hook) = &self.hooks.build_end {
            hook.call(PluginContext {
                context: Arc::downgrade(context),
            })?
        }
        Ok(())
    }

    fn watch_changes(&self, id: &str, event: &str, context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.watch_changes {
            hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                id.to_string(),
                WatchChangesParams {
                    event: event.to_string(),
                },
            ))?
        }
        Ok(())
    }

    fn write_bundle(&self, context: &Arc<Context>) -> Result<()> {
        if let Some(hook) = &self.hooks.write_bundle {
            hook.call(PluginContext {
                context: Arc::downgrade(context),
            })?
        }
        Ok(())
    }

    fn before_write_fs(
        &self,
        path: &std::path::Path,
        content: &[u8],
        context: &Arc<Context>,
    ) -> Result<()> {
        if let Some(hook) = &self.hooks._on_generate_file {
            hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                WriteFile {
                    path: path.to_string_lossy().to_string(),
                    content: content.to_vec(),
                },
            ))?;
        }
        Ok(())
    }

    fn load_transform(
        &self,
        content: &mut Content,
        path: &str,
        _is_entry: bool,
        context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        if let Some(hook) = &self.hooks.transform_include {
            if hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                path.to_string(),
            ))? == Some(false)
            {
                return Ok(None);
            }
        }

        if let Some(hook) = &self.hooks.transform {
            let content_str = match content {
                Content::Js(js_content) => js_content.content.clone(),
                Content::Css(css_content) => css_content.clone(),
                _ => return Ok(None),
            };

            let result: Option<TransformResult> = hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                content_str,
                path.to_string(),
            ))?;

            if let Some(result) = result {
                return content_from_result(result).map(Some);
            }
        }
        Ok(None)
    }

    fn add_deps(
        &self,
        file: &File,
        _deps: &mut Vec<Dependency>,
        context: &Arc<Context>,
    ) -> Result<()> {
        if let Some(hook) = &self.hooks.add_deps {
            let result: Option<AddDepsResult> = hook.call((
                PluginContext {
                    context: Arc::downgrade(context),
                },
                file.path.to_string_lossy().to_string(),
                _deps.iter().map(|dep| dep.source.clone()).collect(),
            ))?;

            if let Some(result) = result {
                let deps = result.deps;
                let _missing_deps = result.missing_deps;

                deps.iter().enumerate().for_each(|(idx, dep)| {
                    _deps.push(Dependency {
                        source: dep.clone(),
                        order: idx,
                        resolve_type: ResolveType::Require,
                        span: None,
                        resolve_as: None,
                    });
                });
            }
        }

        Ok(())
    }

    fn next_build(&self, _next_build_param: &mako::plugin::NextBuildParam) -> bool {
        if let Some(hook) = &self.hooks.next_build {
            let result: Option<bool> = match hook.call((
                (),
                _next_build_param.current_module.id.clone(),
                _next_build_param
                    .next_file
                    .path
                    .to_string_lossy()
                    .to_string(),
            )) {
                Ok(res) => res,
                Err(_) => return false,
            };

            if let Some(result) = result {
                return result;
            }
        }

        true
    }
}
