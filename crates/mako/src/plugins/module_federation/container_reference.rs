use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use serde::Serialize;

use super::{
    ModuleFederationPlugin, FEDERATION_REMOTE_MODULE_PREFIX, FEDERATION_REMOTE_REFERENCE_PREFIX,
};
use crate::compiler::Context;
use crate::module::FedereationModuleType;
use crate::resolve::{RemoteInfo, ResolverResource};

impl ModuleFederationPlugin {
    pub(super) fn get_container_references_code(&self, context: &Arc<Context>) -> String {
        let module_graph = context.module_graph.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();
        let all_chunks = chunk_graph.get_all_chunks();

        let mut chunk_mapping: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut id_to_external_and_name_mapping: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut id_to_remote_map: HashMap<&str, Vec<RemoteExternal>> = HashMap::new();
        all_chunks.iter().for_each(|c| {
            c.modules.iter().for_each(|m| {
                if let Some(m) = module_graph.get_module(m) {
                    if let Some(info) = m.info.as_ref()
                        && let Some(FedereationModuleType::Remote) = info.federation.as_ref()
                    {
                        {
                            chunk_mapping
                                .entry(c.id.id.as_str())
                                .or_default()
                                .push(m.id.id.as_str());
                        }

                        {
                            let remote_module = m
                                .info
                                .as_ref()
                                .unwrap()
                                .resolved_resource
                                .as_ref()
                                .unwrap()
                                .get_remote_info()
                                .unwrap();
                            let remote_info = id_to_external_and_name_mapping
                                .entry(m.id.id.as_str())
                                .or_default();

                            remote_info.push(&remote_module.share_scope);
                            remote_info.push(&remote_module.sub_path);
                            remote_info.push(&remote_module.external_reference_id);

                            let external_info =
                                id_to_remote_map.entry(m.id.id.as_str()).or_default();

                            external_info.push(RemoteExternal {
                                name: remote_module.name.clone(),
                                external_type: remote_module.external_type.clone(),
                                external_module_id: remote_module.external_reference_id.clone(),
                            });
                        }
                    }
                }
            });
        });

        let chunk_mapping = serde_json::to_string(&chunk_mapping).unwrap();
        let id_to_external_and_name_mapping =
            serde_json::to_string(&id_to_external_and_name_mapping).unwrap();
        let id_to_remote_map = serde_json::to_string(&id_to_remote_map).unwrap();

        format!(
            r#"
/* mako/runtime/federation remotes consume */
!(function() {{
  var chunkMapping = {chunk_mapping};
  var idToExternalAndNameMapping = {id_to_external_and_name_mapping};
  var idToRemoteMap = {id_to_remote_map};
  requireModule.federation.bundlerRuntimeOptions.remotes = {{idToRemoteMap, chunkMapping, idToExternalAndNameMapping, webpackRequire: requireModule}};
  requireModule.chunkEnsures.remotes = (chunkId, promises) => {{
    requireModule.federation.bundlerRuntime.remotes({{ idToRemoteMap,chunkMapping, idToExternalAndNameMapping, chunkId, promises, webpackRequire: requireModule}});
  }}
}}
)()"#,
        )
    }

    pub(super) fn resolve_remote(
        &self,
        source: &str,
    ) -> Result<Option<ResolverResource>, anyhow::Error> {
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
                        external_reference_id: format!(
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteExternal {
    external_type: String,
    name: String,
    external_module_id: String,
}
