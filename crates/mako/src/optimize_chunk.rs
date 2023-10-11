use std::collections::HashMap;
use std::string::String;

use indexmap::{IndexMap, IndexSet};
use mako_core::nodejs_resolver::Resource;
use regex::Regex;
use tracing::debug;

use crate::chunk::{Chunk, ChunkId, ChunkType};
use crate::compiler::Compiler;
use crate::module::{Module, ModuleId, ModuleInfo};
use crate::resolve::{ResolvedResource, ResolverResource};

#[derive(Clone)]
pub enum OptimizeAllowChunks {
    // All,
    Entry,
    Async,
}

pub struct OptimizeChunkOptions {
    pub min_size: usize,
    pub groups: Vec<OptimizeChunkGroup>,
}

#[derive(Clone)]
pub struct OptimizeChunkGroup {
    pub name: String,
    pub allow_chunks: OptimizeAllowChunks,
    pub min_chunks: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub test: Option<Regex>,
    pub priority: Option<i8>,
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
            // stage: prepare
            let mut optimize_chunks_infos = optimize_options
                .groups
                .iter()
                .map(|group| OptimizeChunksInfo {
                    group_options: group.clone(),
                    module_to_chunks: IndexMap::new(),
                })
                .collect::<Vec<_>>();

            optimize_chunks_infos.sort_by_key(|o| -o.group_options.priority.unwrap_or(0));

            // stage: deasync
            self.merge_minimal_async_chunks(&optimize_options);

            // stage: modules
            self.module_to_optimize_infos(&mut optimize_chunks_infos);

            // stage: size
            self.optimize_chunk_size(&mut optimize_chunks_infos);

            // stage: apply
            self.apply_optimize_infos(&optimize_chunks_infos);
        }
    }

    fn merge_minimal_async_chunks(&self, options: &OptimizeChunkOptions) {
        let mut async_to_entry = vec![];
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();

        // find minimal async chunks to merge to entry chunk
        for chunk in chunks {
            if chunk.chunk_type == ChunkType::Async && self.get_chunk_size(chunk) < options.min_size
            {
                let entry_ids = chunk_graph.entry_dependencies_chunk(chunk);

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

        for (chunk_id, entry_chunk_id, chunk_modules) in async_to_entry.clone() {
            let entry_chunk = chunk_graph.mut_chunk(&entry_chunk_id).unwrap();

            // merge modules to entry chunk
            for m in chunk_modules {
                entry_chunk.add_module(m.clone());
                merged_modules.push(m);
            }

            // remove original async chunks
            chunk_graph.remove_chunk(&chunk_id);
        }

        // remove merged modules from other async chunks
        let mut chunks = chunk_graph.mut_chunks();

        for chunk in chunks.iter_mut() {
            if chunk.chunk_type == ChunkType::Async {
                chunk.modules.retain(|m| !merged_modules.contains(m));
            }
        }
    }

    fn module_to_optimize_infos<'a>(
        &'a self,
        optimize_chunks_infos: &'a mut Vec<OptimizeChunksInfo>,
    ) {
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();
        let modules_in_chunk = chunks.iter().fold(vec![], |mut acc, chunk| {
            acc.append(
                &mut chunk
                    .modules
                    .iter()
                    .map(|m| (m, &chunk.id, &chunk.chunk_type))
                    .collect::<Vec<_>>(),
            );
            acc
        });
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
                    if !test.is_match(&module_id.id.to_string()) {
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
                    IndexMap::<String, (usize, HashMap<ModuleId, Vec<ChunkId>>)>::new(),
                    |mut size_map, mtc| {
                        let pkg_name = match module_graph.get_module(mtc.0) {
                            Some(Module {
                                info:
                                    Some(ModuleInfo {
                                        resolved_resource:
                                            Some(ResolverResource::Resolved(ResolvedResource(
                                                Resource {
                                                    description: Some(module_desc),
                                                    ..
                                                },
                                            ))),
                                        ..
                                    }),
                                ..
                            }) => module_desc.data().raw().get("name"),
                            _ => None,
                        }
                        .map(|n| n.as_str().unwrap())
                        .unwrap_or("unknown");

                        let module_size = module_graph.get_module(mtc.0).unwrap().get_module_size();

                        // add module size to package size
                        if let Some((item, modules)) = size_map.get_mut(pkg_name) {
                            *item += module_size;
                            modules.insert(mtc.0.clone(), mtc.1.clone());
                        } else {
                            size_map.insert(
                                pkg_name.to_string(),
                                (module_size, HashMap::from([(mtc.0.clone(), mtc.1.clone())])),
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
        let mut edges = HashMap::new();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        for info in optimize_chunks_infos {
            // create new chunk
            let info_chunk_id = ChunkId {
                id: info.group_options.name.clone(),
            };
            let info_chunk = Chunk {
                modules: info
                    .module_to_chunks
                    .keys()
                    .cloned()
                    .collect::<IndexSet<_>>(),
                id: info_chunk_id.clone(),
                chunk_type: ChunkType::Sync,
                content: None,
                source_map: None,
            };
            chunk_graph.add_chunk(info_chunk);

            // remove modules from original chunks and add edge to new chunk
            for (module_id, chunk_ids) in &info.module_to_chunks {
                for chunk_id in chunk_ids {
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
            OptimizeAllowChunks::Entry => matches!(chunk_type, &ChunkType::Entry(_, _)),
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
        match self.context.config.code_splitting {
            crate::config::CodeSplittingStrategy::Auto => Some(OptimizeChunkOptions {
                min_size: 20000,
                groups: vec![
                    OptimizeChunkGroup {
                        name: "vendors".to_string(),
                        allow_chunks: OptimizeAllowChunks::Entry,
                        min_chunks: 1,
                        min_size: 20000,
                        max_size: 5000000,
                        test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                        priority: Some(-10),
                    },
                    OptimizeChunkGroup {
                        name: "vendors_dynamic".to_string(),
                        allow_chunks: OptimizeAllowChunks::Async,
                        min_chunks: 1,
                        min_size: 20000,
                        max_size: 5000000,
                        test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                        priority: Some(-10),
                    },
                    OptimizeChunkGroup {
                        name: "common_dynamic".to_string(),
                        allow_chunks: OptimizeAllowChunks::Async,
                        min_chunks: 2,
                        // always split, to avoid multi-instance risk
                        min_size: 1,
                        max_size: 5000000,
                        test: None,
                        priority: Some(-20),
                    },
                ],
            }),
            _ => None,
        }
    }
}
