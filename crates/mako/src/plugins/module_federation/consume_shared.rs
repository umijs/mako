use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pathdiff::diff_paths;

use super::constants::FEDERATION_SHARED_REFERENCE_PREFIX;
use super::ModuleFederationPlugin;
use crate::build::analyze_deps::{AnalyzeDepsResult, ResolvedDep};
use crate::compiler::Context;
use crate::generate::chunk::{Chunk, ChunkType};
use crate::module::{md5_hash, Dependency, ResolveType};
use crate::plugin::PluginResolveIdParams;
use crate::resolve::{do_resolve, ConsumeSharedInfo, ResolverResource, ResolverType};

impl ModuleFederationPlugin {
    pub(super) fn get_consume_sharing_code(
        &self,
        entry_chunk: &Chunk,
        context: &Context,
    ) -> String {
        let module_graph = context.module_graph.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();
        let share_dependencies = self.provide_shared_map.read().unwrap();

        let consume_modules_chunk_map: HashMap<String, Vec<String>> = chunk_graph
            .installable_descendants_chunk(&entry_chunk.id)
            .into_iter()
            .filter_map(|c| {
                let modules = chunk_graph
                    .chunk(&c)
                    .unwrap()
                    .modules
                    .iter()
                    .filter_map(|m| {
                        if m.id.starts_with(FEDERATION_SHARED_REFERENCE_PREFIX) {
                            Some(m.id.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if modules.is_empty() {
                    None
                } else {
                    Some((c.id, modules))
                }
            })
            .collect();
        let consume_shared_module_ids =
            consume_modules_chunk_map
                .iter()
                .fold(HashSet::<&String>::new(), |mut acc, cur| {
                    acc.extend(cur.1.iter());
                    acc
                });
        let consume_shared_modules = consume_shared_module_ids
            .iter()
            .map(|id| module_graph.get_module(&id.as_str().into()).unwrap())
            .collect::<Vec<_>>();
        let module_to_handler_mapping_code = consume_shared_modules
            .iter()
            .map(|s| {
                let resolved_resource  = s.info.as_ref().unwrap().resolved_resource.as_ref().unwrap();
                let module_full_path = match resolved_resource {
                 ResolverResource::Shared(info) => info.deps.resolved_deps[0].resolver_resource.get_resolved_path(),
                    _ => panic!("{} is not a shared module", resolved_resource.get_resolved_path())
                };
                let module_relative_path =
                    diff_paths(&module_full_path, &context.root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                let module_in_chunk = chunk_graph.get_chunk_for_module(&module_full_path.as_str().into()).unwrap();

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

                let share_dependency = share_dependencies.get(&s.id.id).unwrap();
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

        let chunk_mapping_code = serde_json::to_string(&consume_modules_chunk_map).unwrap();
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

    pub(super) fn resolve_to_consume_share(
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
            let config_joined_str = format!(
                "{}|{}|{}|{}|{}|{}|{}",
                shared_info.shared_scope,
                source,
                shared_info
                    .required_version
                    .as_ref()
                    .map_or("", |v| v.as_str()),
                shared_info.strict_version,
                resolver_resource.get_resolved_path(),
                shared_info.singleton,
                shared_info.eager
            );
            let hash = md5_hash(&config_joined_str, 4);
            return Ok(Some(ResolverResource::Shared(ConsumeSharedInfo {
                eager: shared_info.eager,
                module_id: format!(
                    "{}{}/{}/{}?{}",
                    FEDERATION_SHARED_REFERENCE_PREFIX,
                    shared_info.shared_scope,
                    source,
                    source,
                    hash
                ),
                name: source.to_string(),
                share_scope: shared_info.shared_scope.clone(),
                version: resolver_resource.get_pkg_info().unwrap().version.unwrap(),
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
