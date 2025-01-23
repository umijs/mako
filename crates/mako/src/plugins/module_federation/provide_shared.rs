use pathdiff::diff_paths;
use serde::Serialize;

use super::util::serialize_none_to_false;
use super::ModuleFederationPlugin;
use crate::build::analyze_deps::ResolvedDep;
use crate::compiler::Context;
use crate::generate::chunk::ChunkType;
use crate::module::ModuleId;

impl ModuleFederationPlugin {
    pub(super) fn init_federation_runtime_sharing(&self, context: &Context) -> String {
        let provide_shared_map = self.provide_shared_map.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();

        if provide_shared_map.is_empty() {
            return "".to_string();
        }

        let provide_shared_map_code = format!(
                    r#"{{{}}}"#,
                    provide_shared_map
                        .iter()
                        .filter_map(|(_, share_item)| {
                            let module_id: ModuleId = share_item.file_path.as_str().into();
                            let module_in_chunk = chunk_graph.get_chunk_for_module(&module_id)?;

                            let share_item_code = format!(r#""{share_key}": [{infos}]"#,
                                share_key = share_item.share_key,
                                infos = {
                                    let getter = {
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
                                                    [dependency_chunks, vec![module_in_chunk.id.clone()]]
                                                    .concat().iter().map(|e| format!(r#"requireModule.ensure("{}")"#, e.id))
                                                    .collect::<Vec<String>>().join(",")
                                                )
                                            },
                                            // FIXME:
                                            _ =>  panic!("mf shared dependency should not bundled to worker chunk, entries' shared chunk or runtime chunk")
                                        }
                                    };
                                    format!(
                                        r#"{{ version: "{version}", get: {getter}, scope: {scope}, shareConfig: {share_config} }}"#,
                                        version = share_item.version,
                                        scope = serde_json::to_string(&share_item.scope).unwrap(),
                                        share_config = serde_json::to_string(&share_item.shared_config).unwrap()
                                    )
                                },
                            );
                            Some(share_item_code)}
                        )
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
            provide_shared_map
                .entry(resolved_dep.resolver_resource.get_resolved_path())
                .or_insert(ProvideSharedItem {
                    share_key: pkg_name.clone(),
                    version: pkg_info.version.clone().unwrap(),
                    scope: vec![shared_info.shared_scope.clone()],
                    file_path: pkg_info.file_path.clone(),
                    shared_config: SharedConfig {
                        eager: shared_info.eager,
                        strict_version: shared_info.strict_version,
                        singleton: shared_info.singleton,
                        required_version: shared_info.required_version.clone(),
                        // FIXME: hard code now
                        fixed_dependencies: false,
                    },
                });
        }
    }
}

#[derive(Debug)]
pub(super) struct ProvideSharedItem {
    pub(super) share_key: String,
    pub(super) version: String,
    pub(super) scope: Vec<String>,
    pub(super) shared_config: SharedConfig,
    pub(super) file_path: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct SharedConfig {
    #[serde(default)]
    pub(super) fixed_dependencies: bool,
    pub(super) eager: bool,
    pub(super) strict_version: bool,
    pub(super) singleton: bool,
    #[serde(serialize_with = "serialize_none_to_false")]
    pub(super) required_version: Option<String>,
}
