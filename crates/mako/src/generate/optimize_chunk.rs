use std::collections::HashMap;
use std::string::String;

use mako_core::indexmap::{IndexMap, IndexSet};
use mako_core::regex::Regex;
use mako_core::tracing::debug;

use crate::compiler::Compiler;
use crate::config::{OptimizeAllowChunks, OptimizeChunkGroup, OptimizeChunkOptions};
use crate::generate::chunk::{Chunk, ChunkId, ChunkType};
use crate::generate::group_chunk::GroupUpdateResult;
use crate::module::{Module, ModuleId, ModuleInfo};
use crate::resolve::{ResolvedResource, ResolverResource};

pub(crate) fn default_min_size() -> usize {
    20000
}

pub(crate) fn default_max_size() -> usize {
    5000000
}

pub(crate) fn default_min_chunks() -> usize {
    1
}

pub struct OptimizeChunksInfo {
    pub group_options: OptimizeChunkGroup,
    pub module_to_chunks: IndexMap<ModuleId, Vec<ChunkId>>,
}

#[derive(PartialEq, Eq, Clone)]
pub struct OptimizeChunkModule {
    pub module_id: ModuleId,
    pub chunk_ids: Vec<ChunkId>,
}

impl Compiler {
    pub fn optimize_chunk(&self) {
        mako_core::mako_profile_function!();
        debug!("optimize chunk");
        if let Some(optimize_options) = self.get_optimize_chunk_options() {
            debug!("optimize options: {:?}", optimize_options);
            // stage: prepare
            let mut optimize_chunks_infos = optimize_options
                .groups
                .iter()
                .map(|group| OptimizeChunksInfo {
                    group_options: group.clone(),
                    module_to_chunks: IndexMap::new(),
                })
                .collect::<Vec<_>>();

            optimize_chunks_infos.sort_by_key(|o| -o.group_options.priority);

            // stage: deasync
            self.merge_minimal_async_chunks(&optimize_options);

            // stage: modules
            self.module_to_optimize_infos(&mut optimize_chunks_infos, None);

            // stage: size
            self.optimize_chunk_size(&mut optimize_chunks_infos);

            // stage: apply
            self.apply_optimize_infos(&optimize_chunks_infos);

            // save optimize infos for hot update
            if let Ok(optimize_info) = &mut self.context.optimize_infos.lock() {
                **optimize_info = Some(optimize_chunks_infos);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn optimize_hot_update_chunk(&self, group_result: &GroupUpdateResult) {
        mako_core::mako_profile_function!();
        debug!("optimize hot update chunk");

        // skip if code splitting disabled or group result is invalid
        if self.context.config.code_splitting.is_none() || group_result.is_none() {
            return;
        }

        let (group_new_chunks, group_modules_in_chunk) = group_result.as_ref().unwrap();

        if group_new_chunks.is_empty() && group_modules_in_chunk.is_empty() {
            // full re-optimize if code splitting enabled and received empty group result
            // ref: https://github.com/umijs/mako/blob/d110cbd74e95307c437471185d734e10533b3494/crates/mako/src/group_chunk.rs#L182
            self.optimize_chunk();
        } else if let Some(optimize_infos) = self.context.optimize_infos.lock().unwrap().as_ref() {
            // only optimize if code splitting enabled and there has valid group update result
            let chunk_graph = self.context.chunk_graph.write().unwrap();
            // prepare modules_in_chunk data
            let mut modules_in_chunk = group_new_chunks.iter().fold(vec![], |mut acc, chunk_id| {
                let chunk = chunk_graph.chunk(chunk_id).unwrap();
                acc.append(
                    &mut chunk
                        .modules
                        .iter()
                        .map(|m| (m.clone(), chunk.id.clone(), chunk.chunk_type.clone()))
                        .collect::<Vec<_>>(),
                );
                acc
            });
            modules_in_chunk.extend(group_modules_in_chunk.clone());
            let modules_in_chunk = modules_in_chunk
                .iter()
                .map(|(m, c, t)| (m, c, t))
                .collect::<Vec<_>>();
            drop(chunk_graph);

            // clone an empty optimize infos for hot update
            let mut optimize_infos = optimize_infos
                .iter()
                .map(|info| OptimizeChunksInfo {
                    group_options: info.group_options.clone(),
                    module_to_chunks: IndexMap::new(),
                })
                .collect::<Vec<_>>();

            // stage: modules
            self.module_to_optimize_infos(&mut optimize_infos, Some(modules_in_chunk));

            // stage: apply
            self.apply_hot_update_optimize_infos(&optimize_infos);
        }
    }

    fn merge_minimal_async_chunks(&self, options: &OptimizeChunkOptions) {
        let mut async_to_entry = vec![];
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_all_chunks();

        // find minimal async chunks to merge to entry chunk
        // TODO: continue to merge deep-level async chunk
        for chunk in chunks {
            if chunk.chunk_type == ChunkType::Async && self.get_chunk_size(chunk) < options.min_size
            {
                let entry_ids = chunk_graph.entry_dependents_chunk(&chunk.id);

                // merge if there is only one entry chunk
                // TODO: don't merge if entry chunk size is greater than max_size
                if entry_ids.len() == 1 {
                    async_to_entry.push((
                        chunk.id.clone(),
                        entry_ids[0].clone(),
                        chunk.modules.iter().cloned().collect::<Vec<_>>(),
                    ));
                }
            }
        }
        drop(chunk_graph);

        // update chunk_graph
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        let mut merged_modules = vec![];

        for (index, (chunk_id, entry_chunk_id, chunk_modules)) in async_to_entry.iter().enumerate()
        {
            let entry_chunk: &mut Chunk = chunk_graph.mut_chunk(entry_chunk_id).unwrap();

            // merge modules to entry chunk
            for m in chunk_modules {
                entry_chunk.add_module(m.clone());
                merged_modules.push(m);
            }

            // remove original async chunks
            chunk_graph.remove_chunk(chunk_id);

            // connect that has be optimized chunk dependents to the entry chunk
            if index == async_to_entry.len() - 1 {
                chunk_graph.connect_isolated_nodes_to_chunk(entry_chunk_id);
            }
        }

        // remove merged modules from other async chunks
        let mut chunks = chunk_graph.mut_chunks();

        for chunk in chunks.iter_mut() {
            if chunk.chunk_type == ChunkType::Async {
                chunk.modules.retain(|m| !merged_modules.contains(&m));
            }
        }
    }

    fn module_to_optimize_infos<'a>(
        &'a self,
        optimize_chunks_infos: &'a mut Vec<OptimizeChunksInfo>,
        modules_in_chunk: Option<Vec<(&ModuleId, &ChunkId, &ChunkType)>>,
    ) {
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_all_chunks();
        let async_chunk_root_modules = chunks
            .iter()
            .filter_map(|chunk| match chunk.chunk_type {
                ChunkType::Async => chunk.modules.last(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let modules_in_chunk = match modules_in_chunk {
            Some(modules_in_chunk) => modules_in_chunk,
            None => chunks.iter().fold(vec![], |mut acc, chunk| {
                acc.append(
                    &mut chunk
                        .modules
                        .iter()
                        .filter_map(|m| {
                            match (&chunk.chunk_type, async_chunk_root_modules.contains(&m)) {
                                // async chunk root module should not be optimized
                                (_, true) => None,
                                // entry module of entry chunk should not be optimized
                                (ChunkType::Entry(entry_id, _, false), _)
                                    if m.id == entry_id.id =>
                                {
                                    None
                                }
                                _ => Some((m, &chunk.id, &chunk.chunk_type)),
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                acc
            }),
        };
        for (module_id, chunk_id, chunk_type) in modules_in_chunk {
            for optimize_info in &mut *optimize_chunks_infos {
                // save chunk to optimize info if module already exists in current info
                if let Some(module_to_chunk) = optimize_info.module_to_chunks.get_mut(module_id) {
                    module_to_chunk.push(chunk_id.clone());
                    break;
                }

                // otherwise, check conditions to decide whether to add module to optimize info
                // check allow chunks
                if !self
                    .check_chunk_type_allow(&optimize_info.group_options.allow_chunks, chunk_type)
                {
                    continue;
                }

                // check test regex
                if let Some(test) = &optimize_info.group_options.test {
                    if !test.is_match(&module_id.id) {
                        continue;
                    }
                }

                // check min shared count of chunks
                if optimize_info.group_options.min_chunks > 1
                    && chunks
                        .iter()
                        .filter(|chunk| {
                            chunk.has_module(module_id)
                                && self.check_chunk_type_allow(
                                    &optimize_info.group_options.allow_chunks,
                                    &chunk.chunk_type,
                                )
                        })
                        .take(optimize_info.group_options.min_chunks)
                        .count()
                        != optimize_info.group_options.min_chunks
                {
                    continue;
                }

                // add new module_to_chunk map to optimize info
                optimize_info
                    .module_to_chunks
                    .insert(module_id.clone(), vec![chunk_id.clone()]);
                break;
            }
        }
    }

    fn optimize_chunk_size<'a>(&'a self, optimize_chunks_infos: &'a mut Vec<OptimizeChunksInfo>) {
        let chunk_size_map = optimize_chunks_infos
            .iter()
            .map(|info| {
                let info_chunk = &Chunk {
                    modules: info
                        .module_to_chunks
                        .keys()
                        .cloned()
                        .collect::<IndexSet<_>>(),
                    id: ChunkId { id: "".to_string() },
                    chunk_type: ChunkType::Sync,
                    content: None,
                    source_map: None,
                };

                (
                    info.group_options.name.clone(),
                    self.get_chunk_size(info_chunk),
                )
            })
            .collect::<HashMap<_, _>>();

        // drop optimize infos if chunk size is less than min_size
        optimize_chunks_infos.retain(|info| {
            *chunk_size_map.get(&info.group_options.name).unwrap() >= info.group_options.min_size
        });

        // continue split chunk if chunk size is greater than max_size
        let mut extra_optimize_infos = vec![];
        let module_graph = self.context.module_graph.read().unwrap();
        for info in &mut *optimize_chunks_infos {
            let mut split_chunk_count = 0;
            let mut chunk_size = *chunk_size_map.get(&info.group_options.name).unwrap();

            if chunk_size > info.group_options.max_size {
                let chunk_modules = &info.module_to_chunks;
                // group size by package name
                let mut package_size_map = chunk_modules.iter().fold(
                    IndexMap::<String, (usize, IndexMap<ModuleId, Vec<ChunkId>>)>::new(),
                    |mut size_map, mtc| {
                        let pkg_name = match module_graph.get_module(mtc.0) {
                            Some(Module {
                                info:
                                    Some(ModuleInfo {
                                        resolved_resource:
                                            Some(ResolverResource::Resolved(ResolvedResource(
                                                resolution,
                                            ))),
                                        ..
                                    }),
                                ..
                            }) => resolution.package_json().map_or("unknown", |json| {
                                json.raw_json()
                                    .get("name")
                                    .map_or("unknown", |n| n.as_str().unwrap())
                            }),
                            _ => "unknown",
                        };

                        let module_size = module_graph.get_module(mtc.0).unwrap().get_module_size();

                        // add module size to package size
                        if let Some((item, modules)) = size_map.get_mut(pkg_name) {
                            *item += module_size;
                            modules.insert(mtc.0.clone(), mtc.1.clone());
                        } else {
                            size_map.insert(
                                pkg_name.to_string(),
                                (
                                    module_size,
                                    IndexMap::from([(mtc.0.clone(), mtc.1.clone())]),
                                ),
                            );
                        }
                        size_map
                    },
                );

                // split new chunks until chunk size is less than max_size and there has more than 1 package can be split
                while chunk_size > info.group_options.max_size && package_size_map.len() > 1 {
                    let mut new_chunk_size = 0;
                    let mut new_module_to_chunks = IndexMap::new();

                    // collect modules by package name until chunk size is very to max_size
                    // `new_chunk_size == 0` 用于解决单个 pkg 大小超过 max_size 会死循环的问题
                    while !package_size_map.is_empty()
                        && (new_chunk_size == 0
                            || new_chunk_size + package_size_map.get_index(0).unwrap().1 .0
                                < info.group_options.max_size)
                    {
                        let (_, (size, modules)) = package_size_map.swap_remove_index(0).unwrap();

                        new_chunk_size += size;
                        new_module_to_chunks.extend(modules);
                    }

                    // clone group options for new chunk
                    let mut new_chunk_group_options = info.group_options.clone();
                    new_chunk_group_options.name =
                        format!("{}_{}", info.group_options.name, split_chunk_count);

                    // update original chunk size and split chunk count
                    chunk_size -= new_chunk_size;
                    split_chunk_count += 1;

                    // move modules to new chunk
                    info.module_to_chunks
                        .retain(|module_id, _| !new_module_to_chunks.contains_key(module_id));
                    extra_optimize_infos.push(OptimizeChunksInfo {
                        group_options: new_chunk_group_options,
                        module_to_chunks: new_module_to_chunks,
                    });
                }

                // rename original chunk if it has been split
                if split_chunk_count > 0 {
                    info.group_options.name =
                        format!("{}_{}", info.group_options.name, split_chunk_count);
                }
            }
        }

        // add extra optimize infos
        optimize_chunks_infos.extend(extra_optimize_infos);
    }

    fn apply_optimize_infos(&self, optimize_chunks_infos: &Vec<OptimizeChunksInfo>) {
        let mut edges_map: HashMap<ModuleId, IndexSet<ModuleId>> = HashMap::new();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();

        for info in optimize_chunks_infos {
            // create new chunk
            let info_chunk_id = ChunkId {
                id: info.group_options.name.clone(),
            };
            let info_chunk_type =
                if matches!(info.group_options.allow_chunks, OptimizeAllowChunks::Async) {
                    ChunkType::Sync
                } else {
                    ChunkType::Entry(info_chunk_id.clone(), info.group_options.name.clone(), true)
                };
            let info_chunk = Chunk {
                modules: info
                    .module_to_chunks
                    .keys()
                    .cloned()
                    .collect::<IndexSet<_>>(),
                id: info_chunk_id.clone(),
                chunk_type: info_chunk_type,
                content: None,
                source_map: None,
            };
            chunk_graph.add_chunk(info_chunk);

            // remove modules from original chunks and add edge to new chunk
            for (module_id, chunk_ids) in &info.module_to_chunks {
                for chunk_id in chunk_ids {
                    let chunk = chunk_graph.mut_chunk(chunk_id).unwrap();

                    chunk.remove_module(module_id);

                    // record edge between original chunk and new dependency chunks
                    if let Some(value) = edges_map.get_mut(chunk_id) {
                        value.insert(info_chunk_id.clone());
                    } else {
                        edges_map.insert(chunk_id.clone(), IndexSet::from([info_chunk_id.clone()]));
                    }
                }
            }
        }

        // add edge to original chunks
        for (from, to) in edges_map
            .iter()
            .flat_map(|(from, tos)| tos.iter().map(move |to| (from, to)))
        {
            chunk_graph.add_edge(from, to);
        }
    }

    fn apply_hot_update_optimize_infos(&self, optimize_chunks_infos: &Vec<OptimizeChunksInfo>) {
        let mut edges = HashMap::new();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        for info in optimize_chunks_infos {
            // update group chunk
            for (module_id, chunk_ids) in &info.module_to_chunks {
                // get chunk
                let info_chunk = chunk_graph
                    .mut_chunk(&ChunkId {
                        id: info.group_options.name.clone(),
                    })
                    .unwrap();
                let info_chunk_id = info_chunk.id.clone();

                // append new module
                if !info_chunk.has_module(module_id) {
                    info_chunk.add_module(module_id.clone());
                }

                // remove modules from original chunks and add edge to new chunk
                for chunk_id in chunk_ids.iter().filter(|c| c.id != info_chunk_id.id) {
                    let chunk = chunk_graph.mut_chunk(chunk_id).unwrap();

                    chunk.remove_module(module_id);
                    edges.insert(chunk_id.clone(), info_chunk_id.clone());
                }
            }

            // add edge to original chunks
            for (from, to) in edges.iter() {
                chunk_graph.add_edge(from, to);
            }
        }
    }

    /* the following is util methods */

    fn check_chunk_type_allow(
        &self,
        allow_chunks: &OptimizeAllowChunks,
        chunk_type: &ChunkType,
    ) -> bool {
        match allow_chunks {
            OptimizeAllowChunks::All => matches!(
                chunk_type,
                &ChunkType::Entry(_, _, false) | &ChunkType::Async
            ),
            OptimizeAllowChunks::Entry => matches!(chunk_type, &ChunkType::Entry(_, _, false)),
            OptimizeAllowChunks::Async => chunk_type == &ChunkType::Async,
        }
    }

    fn get_chunk_size(&self, chunk: &Chunk) -> usize {
        let module_graph = self.context.module_graph.read().unwrap();
        let modules = &chunk.modules;

        modules.iter().fold(0, |acc, m| {
            acc + module_graph.get_module(m).unwrap().get_module_size()
        })
    }

    fn get_optimize_chunk_options(&self) -> Option<OptimizeChunkOptions> {
        match &self.context.config.code_splitting {
            Some(crate::config::CodeSplittingStrategy::Auto) => Some(OptimizeChunkOptions {
                groups: vec![
                    OptimizeChunkGroup {
                        name: "vendors".to_string(),
                        test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                        priority: -10,
                        ..Default::default()
                    },
                    OptimizeChunkGroup {
                        name: "common".to_string(),
                        min_chunks: 2,
                        // always split, to avoid multi-instance risk
                        min_size: 1,
                        priority: -20,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            Some(crate::config::CodeSplittingStrategy::Advanced(options)) => Some(options.clone()),
            _ => None,
        }
    }
}
