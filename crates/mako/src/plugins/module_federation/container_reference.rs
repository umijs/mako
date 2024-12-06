use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

use super::ModuleFederationPlugin;
use crate::compiler::Context;

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
                    if m.is_remote {
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
                            remote_info.push(&remote_module.external_refenrence_id);

                            let external_info =
                                id_to_remote_map.entry(m.id.id.as_str()).or_default();

                            external_info.push(RemoteExternal {
                                name: remote_module.name.clone(),
                                external_type: remote_module.external_type.clone(),
                                external_module_id: remote_module.external_refenrence_id.clone(),
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteExternal {
    external_type: String,
    name: String,
    external_module_id: String,
}
