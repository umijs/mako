use std::collections::{HashSet, VecDeque};
use std::vec;

use mako_core::anyhow::Result;
use mako_core::tracing::debug;

use crate::chunk::{Chunk, ChunkId, ChunkType};
use crate::chunk_graph::ChunkGraph;
use crate::compiler::Compiler;
use crate::module::{ModuleId, ResolveType};
use crate::update::UpdateResult;

pub type GroupUpdateResult = Option<(Vec<ChunkId>, Vec<(ModuleId, ChunkId, ChunkType)>)>;

impl Compiler {
    // TODO:
    // - 多个 entry 之间的 chunk 共享

    pub fn group_chunk(&self) {
        mako_core::mako_profile_function!();
        debug!("group_chunk");

        let mut visited = HashSet::new();
        let mut edges = vec![];
        let module_graph = self.context.module_graph.read().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        chunk_graph.clear();

        let entries = module_graph.get_entry_modules();
        debug!("entries: {:?}", entries);
        for entry in entries {
            let mut entry_chunk_name = "index";

            for (key, value) in &self.context.config.entry {
                // hmr entry id has query '?hmr'
                if parse_path(&value.to_string_lossy()).unwrap().path
                    == parse_path(&entry.id).unwrap().path
                {
                    entry_chunk_name = key;
                    break;
                }
            }

            let (chunk, dynamic_dependencies, worker_dependencies) = self.create_chunk(
                &entry,
                ChunkType::Entry(entry.clone(), entry_chunk_name.to_string(), false),
                &mut chunk_graph,
                vec![],
            );
            let chunk_name = chunk.filename();
            visited.insert(chunk.id.clone());
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .into_iter()
                    .map(|dep| (chunk.id.clone(), dep.generate(&self.context).into())),
            );
            chunk_graph.add_chunk(chunk);

            // 抽离成两个函数处理动态依赖中可能有 worker 依赖、worker 依赖中可能有动态依赖的复杂情况
            self.handle_dynamic_dependencies(
                &chunk_name,
                dynamic_dependencies,
                &visited,
                &mut edges,
                &mut chunk_graph,
            );
            self.handle_worker_dependencies(
                &chunk_name,
                worker_dependencies,
                &visited,
                &mut edges,
                &mut chunk_graph,
            );
        }

        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }
    }

    fn handle_dynamic_dependencies(
        &self,
        chunk_name: &str,
        dynamic_dependencies: Vec<ModuleId>,
        visited: &HashSet<ModuleId>,
        edges: &mut Vec<(ModuleId, ModuleId)>,
        chunk_graph: &mut ChunkGraph,
    ) {
        visit_modules(dynamic_dependencies, Some(visited.clone()), |head| {
            let (chunk, dynamic_dependencies, worker_dependencies) = self.create_chunk(
                head,
                ChunkType::Async,
                chunk_graph,
                vec![chunk_name.to_string()],
            );
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .into_iter()
                    .map(|dep| (chunk.id.clone(), dep.generate(&self.context).into())),
            );
            chunk_graph.add_chunk(chunk);
            self.handle_worker_dependencies(
                chunk_name,
                worker_dependencies,
                visited,
                edges,
                chunk_graph,
            );

            dynamic_dependencies
        });
    }

    fn handle_worker_dependencies(
        &self,
        chunk_name: &str,
        worker_dependencies: Vec<ModuleId>,
        visited: &HashSet<ModuleId>,
        edges: &mut Vec<(ModuleId, ModuleId)>,
        chunk_graph: &mut ChunkGraph,
    ) {
        visit_modules(worker_dependencies, Some(visited.clone()), |head| {
            let (chunk, dynamic_dependencies, worker_dependencies) = self.create_chunk(
                head,
                ChunkType::Worker(head.clone()),
                chunk_graph,
                vec![chunk_name.to_string()],
            );
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .into_iter()
                    .map(|dep| (chunk.id.clone(), dep.generate(&self.context).into())),
            );
            chunk_graph.add_chunk(chunk);
            self.handle_dynamic_dependencies(
                chunk_name,
                dynamic_dependencies,
                visited,
                edges,
                chunk_graph,
            );

            worker_dependencies
        });
    }

    pub fn group_hot_update_chunk(&self, update_result: &UpdateResult) -> GroupUpdateResult {
        mako_core::mako_profile_function!();
        debug!("group_hot_update_chunk");

        // unique for queried file modules
        let modified_files = update_result
            .modified
            .iter()
            // ex. ["a.module.css?modules", "a.module.css?asmodule"] => ["a.module.css"]
            .map(|m| m.id.split('?').next().unwrap())
            .collect::<HashSet<_>>();

        // for logic simplicity, full re-group if modified files are more than 1
        // ex. git checkout another branch
        if modified_files.len() > 1 {
            self.group_chunk();
            // empty vec means full re-group
            return Some((vec![], vec![]));
        }

        let mut chunk_graph = self.context.chunk_graph.write().unwrap();

        // handle removed modules
        if !update_result.removed.is_empty() {
            // remove chunk if it is the entry module of chunk
            for module_id in &update_result.removed {
                let chunk_id = ChunkId {
                    id: module_id.generate(&self.context),
                };

                if let Some(chunk) = chunk_graph.chunk(&chunk_id) {
                    let dependent_chunks = chunk_graph.dependents_chunk(&chunk.id);

                    // remove edge for dependent chunks
                    for dependent_id in dependent_chunks {
                        chunk_graph.remove_edge(&dependent_id, &chunk_id);
                    }
                    // remove self
                    chunk_graph.remove_chunk(&chunk_id);
                }
            }

            // remove module if it exists in other chunks
            let mut chunks = chunk_graph.mut_chunks();

            // TODO: skip removed chunk modules above
            for module_id in &update_result.removed {
                for chunk in chunks.iter_mut() {
                    if chunk.has_module(module_id) {
                        chunk.remove_module(module_id);
                    }
                }
            }
        }

        // handle added modules
        if !update_result.added.is_empty() && !update_result.modified.is_empty() {
            // NOTE: currently we only support single modified module
            let first_modified_module: &ModuleId = update_result.modified.iter().next().unwrap();

            // add new modules for dependent chunks of modified module
            let modules_in_chunk =
                self.hot_update_module_chunks(first_modified_module, &mut chunk_graph);

            // collect added async chunks modules from module_graph
            let module_graph = self.context.module_graph.read().unwrap();
            let async_chunk_modules = update_result
                .added
                .iter()
                .filter(|module_id| {
                    module_graph
                        .get_dependents(module_id)
                        .iter()
                        .any(|(_, dep)| dep.resolve_type == ResolveType::DynamicImport)
                })
                .cloned()
                .collect::<_>();

            // create chunk for added async module
            let entry_module_chunk_names =
                self.get_module_entry_chunk_names(first_modified_module, &chunk_graph);
            let new_async_chunks = self.create_update_async_chunks(
                async_chunk_modules,
                &mut chunk_graph,
                entry_module_chunk_names,
            );

            return Some((new_async_chunks, modules_in_chunk));
        }

        None
    }

    fn get_module_chunks(
        &self,
        module_id: &ModuleId,
        chunk_graph: &ChunkGraph,
    ) -> Vec<(ModuleId, String)> {
        let chunks = chunk_graph.get_all_chunks();

        chunks
            .iter()
            .filter(|chunk| chunk.has_module(module_id))
            .map(|chunk| (chunk.id.clone(), chunk.filename()))
            .collect::<Vec<_>>()
    }

    fn get_module_entry_chunk_names(
        &self,
        module_id: &ModuleId,
        chunk_graph: &ChunkGraph,
    ) -> Vec<String> {
        let module_chunks = self.get_module_chunks(module_id, chunk_graph);
        let mut ret = vec![];

        for (chunk_id, _) in &module_chunks {
            let chunk = chunk_graph.chunk(chunk_id).unwrap();

            if let ChunkType::Entry(_, _, _) = chunk.chunk_type {
                ret.push(chunk.filename());
            } else {
                for chunk_id in chunk_graph.entry_ancestors_chunk(&chunk.id) {
                    ret.push(chunk_graph.chunk(&chunk_id).unwrap().filename());
                }
            }
        }
        ret
    }

    fn hot_update_module_chunks(
        &self,
        modified_module_id: &ModuleId,
        chunk_graph: &mut ChunkGraph,
    ) -> Vec<(ModuleId, ChunkId, ChunkType)> {
        mako_core::mako_profile_function!(&modified_module_id.id);
        let module_graph = self.context.module_graph.read().unwrap();
        let module_chunks = self.get_module_chunks(modified_module_id, chunk_graph);
        let shared_chunk_names = self.get_module_entry_chunk_names(modified_module_id, chunk_graph);
        let mut modules_in_chunk = vec![];

        visit_modules(vec![modified_module_id.clone()], None, |head| {
            // visit all static deps for modified module
            let static_deps = module_graph
                .get_dependencies(head)
                .into_iter()
                .filter(|(_, dep)| {
                    dep.resolve_type != ResolveType::DynamicImport
                        && dep.resolve_type != ResolveType::Worker
                })
                .collect::<Vec<_>>();
            let mut next_module_ids = vec![];

            for (dep_module_id, _dep) in static_deps {
                let module_already_in_entry = shared_chunk_names.iter().any(|name| {
                    chunk_graph
                        .get_chunk_by_name(name)
                        .unwrap()
                        .has_module(dep_module_id)
                });

                // skip shared module with entry
                if !module_already_in_entry {
                    let mut is_new_module = false;

                    // add new module to all parent chunks
                    for (chunk_id, _) in &module_chunks {
                        let module_chunk = chunk_graph.mut_chunk(chunk_id).unwrap();

                        if !module_chunk.has_module(dep_module_id) {
                            // TODO: css module order
                            module_chunk.add_module(dep_module_id.clone());
                            modules_in_chunk.push((
                                dep_module_id.clone(),
                                module_chunk.id.clone(),
                                module_chunk.chunk_type.clone(),
                            ));
                            is_new_module = true;
                        }
                    }

                    // continue to visit child deps, if current dep module is an new module for parent chunks
                    if is_new_module {
                        next_module_ids.push(dep_module_id.clone());
                    }
                }
            }

            next_module_ids
        });

        modules_in_chunk
    }

    fn is_entry_shared_module(
        &self,
        module_id: &ModuleId,
        shared_chunk_names: &[String],
        chunk_graph: &ChunkGraph,
    ) -> bool {
        shared_chunk_names.iter().any(|name| {
            chunk_graph
                .get_chunk_by_name(name)
                .unwrap()
                .has_module(module_id)
        })
    }

    fn create_chunk(
        &self,
        entry_module_id: &ModuleId,
        chunk_type: ChunkType,
        chunk_graph: &mut ChunkGraph,
        shared_chunk_names: Vec<String>,
    ) -> (Chunk, Vec<ModuleId>, Vec<ModuleId>) {
        mako_core::mako_profile_function!(&entry_module_id.id);
        let mut dynamic_entries = vec![];
        let mut worker_entries = vec![];

        let chunk_id = entry_module_id.generate(&self.context);
        let mut chunk = Chunk::new(chunk_id.into(), chunk_type.clone());
        let mut visited_modules: Vec<ModuleId> = vec![entry_module_id.clone()];

        let module_graph = self.context.module_graph.read().unwrap();

        visit_modules(vec![entry_module_id.clone()], None, |head| {
            let parent_index = visited_modules
                .iter()
                .position(|m| m.id == head.id)
                .unwrap_or(0);
            let mut normal_deps = vec![];
            let mut next_module_ids = vec![];

            for (dep_module_id, dep) in module_graph.get_dependencies(head) {
                match dep.resolve_type {
                    ResolveType::DynamicImport => {
                        dynamic_entries.push(dep_module_id.clone());
                    }
                    ResolveType::Worker => {
                        worker_entries.push(dep_module_id.clone());
                    }
                    // skip shared modules from entry chunks, but except worker chunk modules
                    _ if matches!(chunk_type, ChunkType::Worker(_))
                        || !self.is_entry_shared_module(
                            dep_module_id,
                            &shared_chunk_names,
                            chunk_graph,
                        ) =>
                    {
                        next_module_ids.push(dep_module_id.clone());
                        // collect normal deps for current head
                        normal_deps.push(dep_module_id.clone());
                    }
                    _ => {}
                }
            }

            // insert normal deps before head, so that we can keep the dfs order
            visited_modules.splice(parent_index..parent_index, normal_deps);

            next_module_ids
        });

        // add modules to chunk as dfs order
        for module_id in visited_modules {
            chunk.add_module(module_id);
        }

        (chunk, dynamic_entries, worker_entries)
    }

    fn create_update_async_chunks(
        &self,
        async_module_ids: Vec<ModuleId>,
        chunk_graph: &mut ChunkGraph,
        shared_chunk_names: Vec<String>,
    ) -> Vec<ChunkId> {
        let mut edges = vec![];
        let mut new_chunks = vec![];

        visit_modules(async_module_ids, None, |head| {
            let (new_chunk, dynamic_dependencies, worker_dependencies) = self.create_chunk(
                head,
                ChunkType::Async,
                chunk_graph,
                shared_chunk_names.clone(),
            );
            let chunk_id = new_chunk.id.clone();

            // record edges and add chunk to graph
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .into_iter()
                    .map(|dep| {
                        (
                            chunk_id.clone(),
                            match chunk_graph.get_chunk_for_module(&dep) {
                                // ref existing chunk
                                Some(chunk) => chunk.id.clone(),
                                // ref new chunk
                                None => dep.generate(&self.context).into(),
                            },
                        )
                    }),
            );
            chunk_graph.add_chunk(new_chunk);
            new_chunks.push(chunk_id.clone());

            // continue to visit non-existing dynamic dependencies
            dynamic_dependencies
                .into_iter()
                .filter(|dep| chunk_graph.get_chunk_for_module(dep).is_none())
                .collect::<Vec<ModuleId>>()
        });

        // add edges
        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }

        new_chunks
    }
}

fn visit_modules<F>(ids: Vec<ModuleId>, visited: Option<HashSet<ModuleId>>, mut callback: F)
where
    F: FnMut(&ModuleId) -> Vec<ModuleId>,
{
    let mut queue = VecDeque::from(ids);
    let mut visited = visited.unwrap_or_default();

    while let Some(id) = queue.pop_front() {
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        queue.extend(callback(&id));
    }
}

// TODO: REMOVE THIS
fn parse_path(path: &str) -> Result<FileRequest> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    Ok(FileRequest {
        path: path.to_string(),
    })
}

#[derive(Debug, Clone)]
struct FileRequest {
    pub path: String,
}
