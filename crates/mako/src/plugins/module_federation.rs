use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use constants::{FEDERATION_REMOTE_MODULE_PREFIX, FEDERATION_REMOTE_REFERENCE_PREFIX};
use provide_shared::SharedDependencies;

use crate::ast::file::Content;
use crate::build::analyze_deps::ResolvedDep;
use crate::compiler::{Args, Context};
use crate::config::module_federation::ModuleFederationConfig;
use crate::config::Config;
use crate::plugin::{Plugin, PluginGenerateEndParams, PluginResolveIdParams};
use crate::resolve::ResolverResource;

mod constants;
mod container;
mod container_reference;
mod manifest;
mod provide_shared;
mod util;

pub struct ModuleFederationPlugin {
    pub config: ModuleFederationConfig,
    shared_dependencies: RwLock<SharedDependencies>,
}

impl ModuleFederationPlugin {
    pub fn new(config: ModuleFederationConfig) -> Self {
        Self {
            config,
            shared_dependencies: RwLock::new(SharedDependencies::new()),
        }
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
        content: &mut Content,
        _path: &str,
        _is_entry: bool,
        context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        // add container entry runtime dependency
        if !_is_entry {
            Ok(None)
        } else {
            match content {
                Content::Js(js_content) => {
                    let entry_runtime_dep_path = self.prepare_entry_runtime_dep(&context.root);
                    js_content.content.insert_str(
                        0,
                        format!(
                            r#"import "{}";
"#,
                            entry_runtime_dep_path
                        )
                        .as_str(),
                    );
                    Ok(Some(content.clone()))
                }
                _ => Ok(None),
            }
        }
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        Ok(vec![
            self.get_federation_runtime_code(),
            self.get_container_references_code(context),
            self.get_provide_sharing_code(context),
            self.get_consume_sharing_code(context),
            self.get_federation_exposes_library_code(),
        ])
    }

    fn resolve_id(
        &self,
        source: &str,
        importer: &str,
        params: &PluginResolveIdParams,
        context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        let remote_module = self.resolve_remote(source);
        if let Ok(Some(_)) = remote_module.as_ref() {
            remote_module
        } else {
            self.resolve_provide_share(source, importer, params, context)
        }
    }

    fn generate_end(&self, params: &PluginGenerateEndParams, context: &Arc<Context>) -> Result<()> {
        self.generate_federation_manifest(context, params)?;
        Ok(())
    }

    fn after_resolve(&self, resolved_dep: &ResolvedDep, _context: &Arc<Context>) -> Result<()> {
        self.collect_provide_shared_map(resolved_dep);
        Ok(())
    }
}
