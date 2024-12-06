use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use constants::{FEDERATION_REMOTE_MODULE_PREFIX, FEDERATION_REMOTE_REFERENCE_PREFIX};

use crate::ast::file::Content;
use crate::compiler::{Args, Context};
use crate::config::module_federation::ModuleFederationConfig;
use crate::config::Config;
use crate::plugin::{Plugin, PluginGenerateEndParams, PluginResolveIdParams};
use crate::resolve::{RemoteInfo, ResolverResource};

mod constants;
mod container;
mod container_reference;
mod manifest;
mod util;

pub struct ModuleFederationPlugin {
    pub config: ModuleFederationConfig,
}

impl ModuleFederationPlugin {
    pub fn new(config: ModuleFederationConfig) -> Self {
        Self { config }
    }
}

impl Plugin for ModuleFederationPlugin {
    fn name(&self) -> &str {
        "module_federation"
    }

    fn modify_config(&self, config: &mut Config, root: &Path, _args: &Args) -> Result<()> {
        self.add_container_entry(config, root);

        // self.append_remotes_externals(config);

        Ok(())
    }

    fn load_transform(
        &self,
        _content: &mut Content,
        _path: &str,
        _is_entry: bool,
        _context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        // add containter entry runtime dependency
        if !_is_entry {
            Ok(None)
        } else {
            match _content {
                Content::Js(js_content) => {
                    let entry_runtime_dep_path = self.prepare_entry_runtime_dep(&_context.root);
                    js_content.content.insert_str(
                        0,
                        format!(
                            r#"import "{}";
"#,
                            entry_runtime_dep_path
                        )
                        .as_str(),
                    );
                    Ok(Some(_content.clone()))
                }
                _ => Ok(None),
            }
        }
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let federation_runtime_code = self.get_federation_runtime_code();
        let federation_exposes_library_code = self.get_federation_exposes_library_code();
        let federation_container_references_code = self.get_container_references_code(context);

        Ok(vec![
            federation_runtime_code,
            federation_container_references_code,
            federation_exposes_library_code,
        ])
    }

    fn resolve_id(
        &self,
        source: &str,
        _importer: &str,
        _params: &PluginResolveIdParams,
        _context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        let source_parts = source
            .split_once("/")
            .map_or((source.to_string(), ".".to_string()), |(part_0, part_1)| {
                (part_0.to_string(), part_1.to_string())
            });
        Ok(self.config.remotes.as_ref().map_or_else(
            || None,
            |remotes| {
                remotes.get(&source_parts.0).map(|_remote| {
                    ResolverResource::Remote(RemoteInfo {
                        module_id: format!("{}{}", FEDERATION_REMOTE_MODULE_PREFIX, source),
                        external_refenrence_id: format!(
                            "{}{}",
                            FEDERATION_REMOTE_REFERENCE_PREFIX, source_parts.0
                        ),
                        // FIXME: hard code now
                        external_type: "script".to_string(),
                        sub_path: format!("./{}", source_parts.1),
                        name: source_parts.0.to_string(),
                        share_scope: self.config.share_scope.clone(),
                    })
                })
            },
        ))
    }

    fn generate_end(&self, params: &PluginGenerateEndParams, context: &Arc<Context>) -> Result<()> {
        self.generate_federation_manifest(context, params)?;
        Ok(())
    }
}
