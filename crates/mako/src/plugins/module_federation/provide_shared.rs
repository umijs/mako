use core::panic;

use pathdiff::diff_paths;
use serde::Serialize;

use super::ModuleFederationPlugin;
use crate::build::analyze_deps::ResolvedDep;
use crate::compiler::Context;
use crate::generate::chunk::ChunkType;
use crate::generate::chunk_graph::ChunkGraph;
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

impl ModuleFederationPlugin {
    pub(super) fn get_provide_sharing_code(&self, context: &Context) -> String {
        let provide_shared_map = self.provide_shared_map.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();

        if provide_shared_map.is_empty() {
            return "".to_string();
        }

        let  provide_shared_map_code = format!(
            r#"{{{}}}"#,
            provide_shared_map
                .iter()
                .map(|(_, items)| format!(
                    r#""{share_key}": [{infos}]"#,
                    infos = items
                        .iter()
                        .map(|share_item| {
                            let getter = {
                                let module_id: ModuleId = share_item.file_path.as_str().into();
                                let module_in_chunk =
                                    chunk_graph.get_chunk_for_module(&module_id).unwrap();
                                let module_relative_path =
                                    diff_paths(&share_item.file_path, &context.root)
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string();

                                match &module_in_chunk.chunk_type {
                                    ChunkType::Entry(_, _, false) | ChunkType::Worker(_) => {
                                        format!(
                                            r#"() => (() => requireModule("{module_relative_path}"))"#
                                        )
                                    },
                                    ChunkType::Async
                                    | ChunkType::Sync
                                    | ChunkType::Entry(_, _, true)
                                     => {
                                        let dependency_chunks = chunk_graph.sync_dependencies_chunk(&module_in_chunk.id);
                                        format!(
                                            r#"() => (Promise.all([{}]).then(() => (() => requireModule("{module_relative_path}"))))"#,
                                            [dependency_chunks, vec![module_in_chunk.id.clone()]].concat().iter().map(|e| format!(r#"requireModule.ensure("{}")"#, e.id)).collect::<Vec<String>>().join(",")
                                        )
                                    },
                                    // FIXME:
                                    _ =>  panic!("mf shared dependency should not bundled to worker chunk, entries' shared chunk or runtime chunk")
                                }
                            };
                            format!(
                                r#"{{ version: {version}, get: {getter}, scope: {scope}, shareConfig: {share_config} }}"#,
                                version = share_item
                                    .version
                                    .as_ref()
                                    .map_or("false".to_string(), |v| format!(r#""{}""#, v)),
                                scope = serde_json::to_string(&share_item.scope).unwrap(),
                                share_config = serde_json::to_string(&share_item.shared_config).unwrap()
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(","),
                   share_key = items[0].share_key
                ))
                .collect::<Vec<String>>()
                .join(",")
        );

        if let Some(shared) = self.config.shared.as_ref()
            && !shared.is_empty()
        {
            format!(
                r#"
/* mako/runtime/federation sharing */
!(function() {{
  requireModule.federation.initOptions.shared = {provide_shared_map_code};
  requireModule.S = {{}};
  var initPromises = {{}};
  var initTokens = {{}};
  requireModule.I = function(name, initScope) {{
    return requireModule.federation.bundlerRuntime.I({{
      shareScopeName: name,
      webpackRequire: requireModule,
      initPromises: initPromises,
      initTokens: initTokens,
      initScope: initScope
    }});
  }};
}})();
"#,
            )
        } else {
            "".to_string()
        }
    }

    pub(super) fn collect_provide_shared(&self, resolved_dep: &ResolvedDep) {
        if let Some(shared) = self.config.shared.as_ref()
            && let Some(pkg_info) = resolved_dep.resolver_resource.get_pkg_info()
            && let Some(pkg_name) = pkg_info.name
            && let Some(shared_info) = shared.get(&pkg_name)
            && pkg_name == resolved_dep.dependency.source
        {
            let mut provide_shared_map = self.provide_shared_map.write().unwrap();
            let shared_items = provide_shared_map
                .entry(resolved_dep.resolver_resource.get_resolved_path())
                .or_default();
            if shared_items.is_empty() {
                shared_items.push(ProvideSharedItem {
                    share_key: pkg_name.clone(),
                    version: pkg_info.version.clone(),
                    scope: vec![shared_info.shared_scope.clone()],
                    file_path: pkg_info.file_path.clone(),
                    shared_config: SharedDepency {
                        eager: shared_info.eager,
                        strict_version: shared_info.strict_version,
                        singleton: shared_info.singleton,
                        required_version: pkg_info.version.clone(),
                        fixed_dependencies: false,
                    },
                })
            };
        }
    }

    pub(super) fn connect_provide_shared_to_container(
        &self,
        chunk_graph: &mut ChunkGraph,
        module_graph: &mut ModuleGraph,
    ) {
        let entry_chunks = chunk_graph
            .get_chunks()
            .into_iter()
            .filter_map(|c| {
                if matches!(c.chunk_type, ChunkType::Entry(_, _, false)) {
                    Some(c.id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let provide_shared_map = self.provide_shared_map.read().unwrap();

        let provide_shared_in_chunks = provide_shared_map
            .iter()
            .map(|m| {
                chunk_graph
                    .get_chunk_for_module(&m.0.as_str().into())
                    .unwrap()
                    .id
                    .clone()
            })
            .collect::<Vec<_>>();

        entry_chunks.iter().for_each(|ec| {
            provide_shared_in_chunks.iter().for_each(|c| {
                chunk_graph.add_edge(ec, c);
            });
        });
    }
}

#[derive(Debug)]
pub(super) struct ProvideSharedItem {
    pub(super) share_key: String,
    pub(super) version: Option<String>,
    pub(super) scope: Vec<String>,
    pub(super) shared_config: SharedDepency,
    pub(super) file_path: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedDepency {
    #[serde(default)]
    pub(super) fixed_dependencies: bool,
    pub(super) eager: bool,
    pub(super) strict_version: bool,
    pub(super) singleton: bool,
    pub(super) required_version: Option<String>,
}
