use core::panic;
use std::collections::HashMap;
use std::sync::Arc;

use pathdiff::diff_paths;
use serde::Serialize;

use super::constants::FEDERATION_SHARED_REFERENCE_PREFIX;
use super::ModuleFederationPlugin;
use crate::build::analyze_deps::{AnalyzeDepsResult, ResolvedDep};
use crate::compiler::Context;
use crate::generate::chunk::ChunkType;
use crate::module::{Dependency, ModuleId, ResolveType};
use crate::plugin::PluginResolveIdParams;
use crate::resolve::{do_resolve, ConsumeShareInfo, ResolverResource, ResolverType};

impl ModuleFederationPlugin {
    pub(super) fn get_provide_sharing_code(&self, context: &Context) -> String {
        let provide_shared_map = self.shared_dependencies.read().unwrap();
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

    pub(super) fn get_consume_sharing_code(&self, context: &Context) -> String {
        let module_graph = context.module_graph.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();
        let share_dependencies = self.shared_dependencies.read().unwrap();
        let shared_modules = module_graph
            .modules()
            .into_iter()
            .filter(|m| m.id.id.starts_with(FEDERATION_SHARED_REFERENCE_PREFIX))
            .collect::<Vec<_>>();
        let mut shared_referenced_map: HashMap<String, Vec<String>> = HashMap::new();
        let module_to_handler_mapping_code = shared_modules
            .iter()
            .map(|s| {
                let resolved_resource  = s.info.as_ref().unwrap().resolved_resource.as_ref().unwrap();
                let module_full_path = match resolved_resource {
                 ResolverResource::ConsumeShare(info) => info.deps.resolved_deps[0].resolver_resource.get_resolved_path(),
                    _ => panic!("{} is not a shared module", resolved_resource.get_resolved_path())
                };
                let module_relative_path =
                    diff_paths(&module_full_path, &context.root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                let module_in_chunk = chunk_graph.get_chunk_for_module(&module_full_path.as_str().into()).unwrap();

                chunk_graph.dependents_chunk(&module_in_chunk.id).iter().for_each(|c| {
                   let chunk = chunk_graph.chunk(c);
                        if let Some(chunk) = chunk.as_ref() && !matches!(chunk.chunk_type, ChunkType::Runtime | ChunkType::Entry(_, _, false)) {
                        let entry = shared_referenced_map.entry(c.id.clone()).or_default();
                        entry.push(s.id.id.clone());
                    }
                });

                let getter = match &module_in_chunk.chunk_type {
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
                            r#"() => (Promise.all([{}]).then(() => requireModule("{module_relative_path}")))"#,
                            [dependency_chunks, vec![module_in_chunk.id.clone()]].concat().iter().map(|e| format!(r#"requireModule.ensure("{}")"#, e.id)).collect::<Vec<String>>().join(",")
                        )
                    },
                    ChunkType::Runtime  =>  panic!("mf shared dependency should not bundled to runtime chunk")
                };

                let share_dependency = share_dependencies.get(&s.id.id).unwrap().first().unwrap();
                format!(
                    r#""{shared_consume_id}": {{
    getter: {getter},
    shareInfo: {{ shareConfig: {share_config} }},
    shareKey: "{share_key}"
                    }}"#,
                    shared_consume_id = s.id.id,
                    share_config = serde_json::to_string(&share_dependency.shared_config).unwrap(),
                    share_key = share_dependency.share_key

                )
            })
            .collect::<Vec<String>>()
            .join(",");
        let chunk_mapping_code = serde_json::to_string(&shared_referenced_map).unwrap();
        format!(
            r#"
/* mako/runtime/federation consumes */
!(() => {{
    var installedModules = {{}};
    var moduleToHandlerMapping = {{{module_to_handler_mapping_code}}};

    var chunkMapping = {chunk_mapping_code};
    requireModule.chunkEnsures.consumes = (chunkId, promises) => {{
        requireModule.federation.bundlerRuntime.consumes({{
        chunkMapping: chunkMapping,
        installedModules: installedModules,
        chunkId: chunkId,
        moduleToHandlerMapping: moduleToHandlerMapping,
        promises: promises,
        webpackRequire: requireModule
        }});
    }}
}})();"#
        )
    }

    pub(super) fn collect_provide_shared_map(&self, resolved_dep: &ResolvedDep) {
        if let Some(shared) = self.config.shared.as_ref()
            && let Some(pkg_info) = resolved_dep.resolver_resource.get_pkg_info()
            && let Some(pkg_name) = pkg_info.name
            && let Some(shared_info) = shared.get(&pkg_name)
            && pkg_name == resolved_dep.dependency.source
        {
            let mut provide_shared_map = self.shared_dependencies.write().unwrap();
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

    pub(super) fn resolve_provide_share(
        &self,
        source: &str,
        importer: &str,
        params: &PluginResolveIdParams,
        context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>, anyhow::Error> {
        if let Some(shared) = self.config.shared.as_ref()
            && let Some(shared_info) = shared.get(source)
        {
            let resolver = if params.dep.resolve_type == ResolveType::Require {
                context.resolvers.get(&ResolverType::Cjs)
            } else if params.dep.resolve_type == ResolveType::Css {
                context.resolvers.get(&ResolverType::Css)
            } else {
                context.resolvers.get(&ResolverType::Esm)
            }
            .unwrap();
            let resolver_resource =
                do_resolve(importer, source, resolver, Some(&context.config.externals))?;
            return Ok(Some(ResolverResource::ConsumeShare(ConsumeShareInfo {
                eager: shared_info.eager,
                module_id: format!(
                    "{}{}/{}/{}",
                    FEDERATION_SHARED_REFERENCE_PREFIX, shared_info.shared_scope, source, source
                ),
                name: source.to_string(),
                share_scope: shared_info.shared_scope.clone(),
                version: resolver_resource.get_pkg_info().unwrap().version.unwrap(),
                full_path: format!(
                    "{}{}/{}/{}",
                    FEDERATION_SHARED_REFERENCE_PREFIX, shared_info.shared_scope, source, source
                ),
                deps: AnalyzeDepsResult {
                    resolved_deps: vec![ResolvedDep {
                        resolver_resource,
                        dependency: Dependency {
                            source: params.dep.source.clone(),
                            resolve_as: None,
                            resolve_type: ResolveType::DynamicImport(Default::default()),
                            order: params.dep.order,
                            span: params.dep.span,
                        },
                    }],
                    missing_deps: HashMap::new(),
                },
                singletion: shared_info.singleton,
                required_version: shared_info.required_version.clone(),
                strict_version: shared_info.strict_version,
            })));
        }
        Ok(None)
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

pub(super) type SharedDependencies = HashMap<String, Vec<ProvideSharedItem>>;
