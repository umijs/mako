use std::collections::HashSet;
use std::hash::Hash;
use std::vec;

use tracing::debug;

use crate::ast::file::parse_path;
use crate::compiler::Compiler;
use crate::config::ChunkGroup;
use crate::dev::update::UpdateResult;
use crate::generate::chunk::{Chunk, ChunkId, ChunkType};
use crate::generate::chunk_graph::ChunkGraph;
use crate::module::{generate_module_id, ModuleId, ResolveType};

pub type GroupUpdateResult = Option<(Vec<ChunkId>, Vec<(ModuleId, ChunkId, ChunkType)>)>;

impl Compiler {
    pub fn group_chunk(&self) {
        crate::mako_profile_function!();
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
                if parse_path(&value.to_string_lossy()).unwrap().0
                    == parse_path(&entry.id).unwrap().0
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
                &None,
            );
            let chunk_name = chunk.filename();
            visited.insert((chunk.id.clone(), None));
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .iter()
                    .map(|dep| {
                        (
                            chunk.id.clone(),
                            match dep {
                                (_, Some(chunk_group)) => {
                                    generate_module_id(&chunk_group.name, &self.context).into()
                                }
                                (module_id, None) => module_id.generate(&self.context).into(),
                            },
                        )
                    }),
            );
            chunk_graph.add_chunk(chunk);

            /* A worker can self-spawn from it's source file, that will leads to a circular dependencies,
             * a real case is https://unpkg.com/browse/@antv/layout-wasm@1.4.0/pkg-parallel/.
             * Memorize handled workers to avoid infinite resolving
             */
            let mut visited_workers = HashSet::<(ModuleId, Option<ChunkGroup>)>::new();

            // 抽离成两个函数处理动态依赖中可能有 worker 依赖、worker 依赖中可能有动态依赖的复杂情况
            self.handle_dynamic_dependencies(
                &chunk_name,
                dynamic_dependencies,
                &visited,
                &mut edges,
                &mut chunk_graph,
                &mut visited_workers,
            );
            self.handle_worker_dependencies(
                &chunk_name,
                worker_dependencies,
                &visited,
                &mut edges,
                &mut chunk_graph,
                &mut visited_workers,
            );
        }

        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }
    }

    fn handle_dynamic_dependencies(
        &self,
        chunk_name: &str,
        dynamic_dependencies: Vec<(ModuleId, Option<ChunkGroup>)>,
        visited: &HashSet<(ModuleId, Option<ChunkGroup>)>,
        edges: &mut Vec<(ModuleId, ModuleId)>,
        chunk_graph: &mut ChunkGraph,
        visited_workers: &mut HashSet<(ModuleId, Option<ChunkGroup>)>,
    ) {
        visit_modules(dynamic_dependencies, Some(visited.clone()), |head| {
            let (chunk, dynamic_dependencies, mut worker_dependencies) = self.create_chunk(
                &head.0,
                ChunkType::Async,
                chunk_graph,
                vec![chunk_name.to_string()],
                &head.1,
            );

            worker_dependencies.retain(|w| !visited_workers.contains(w));

            if let Some(exited_chunk) = chunk_graph.mut_chunk(&chunk.id) {
                chunk
                    .modules
                    .iter()
                    .for_each(|m| exited_chunk.add_module(m.clone()));
            } else {
                edges.extend(
                    [dynamic_dependencies.clone(), worker_dependencies.clone()]
                        .concat()
                        .iter()
                        .map(|dep| {
                            (
                                chunk.id.clone(),
                                match dep {
                                    (_, Some(chunk_group)) => {
                                        generate_module_id(&chunk_group.name, &self.context).into()
                                    }
                                    (module_id, None) => module_id.generate(&self.context).into(),
                                },
                            )
                        }),
                );
                chunk_graph.add_chunk(chunk);
            }

            self.handle_worker_dependencies(
                chunk_name,
                worker_dependencies,
                visited,
                edges,
                chunk_graph,
                visited_workers,
            );

            dynamic_dependencies
        });
    }

    fn handle_worker_dependencies(
        &self,
        chunk_name: &str,
        worker_dependencies: Vec<(ModuleId, Option<ChunkGroup>)>,
        visited: &HashSet<(ModuleId, Option<ChunkGroup>)>,
        edges: &mut Vec<(ModuleId, ModuleId)>,
        chunk_graph: &mut ChunkGraph,
        visited_workers: &mut HashSet<(ModuleId, Option<ChunkGroup>)>,
    ) {
        visit_modules(worker_dependencies, Some(visited.clone()), |head| {
            let (chunk, dynamic_dependencies, mut worker_dependencies) = self.create_chunk(
                &head.0,
                ChunkType::Worker(head.0.clone()),
                chunk_graph,
                vec![chunk_name.to_string()],
                &head.1,
            );

            worker_dependencies.retain(|w| !visited_workers.contains(w));

            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .iter()
                    .map(|dep| {
                        (
                            chunk.id.clone(),
                            match dep {
                                (_, Some(chunk_group)) => {
                                    generate_module_id(&chunk_group.name, &self.context).into()
                                }
                                (module_id, None) => module_id.generate(&self.context).into(),
                            },
                        )
                    }),
            );

            chunk_graph.add_chunk(chunk);

            visited_workers.insert(head.clone());

            self.handle_dynamic_dependencies(
                chunk_name,
                dynamic_dependencies,
                visited,
                edges,
                chunk_graph,
                visited_workers,
            );

            worker_dependencies
        });
    }

    pub fn group_hot_update_chunk(&self, update_result: &UpdateResult) -> GroupUpdateResult {
        crate::mako_profile_function!();
        debug!("group_hot_update_chunk");

        // unique for queried file modules
        let modified_files = update_result
            .modified
            .iter()
            // ex. ["a.module.css?modules", "a.module.css?asmodule"] => ["a.module.css"]
            .map(|m| m.id.split('?').next().unwrap())
            .collect::<HashSet<_>>();

        // 1. for logic simplicity, full re-group if modified files are more than 1
        //    ex. git checkout another branch;
        // 2. if dependencies have been changed, should full re-group too.
        if modified_files.len() > 1 || !update_result.dep_changed.is_empty() {
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
                .filter_map(|module_id| {
                    module_graph
                        .get_dependents(module_id)
                        .iter()
                        .find_map(|(_, dep)| match &dep.resolve_type {
                            ResolveType::DynamicImport(chunk_group) => {
                                Some((module_id.clone(), chunk_group.clone()))
                            }
                            _ => None,
                        })
                })
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
        crate::mako_profile_function!(&modified_module_id.id);
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
                    !matches!(
                        dep.resolve_type,
                        ResolveType::DynamicImport(_) | ResolveType::Worker(_)
                    )
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

    #[allow(clippy::type_complexity)]
    fn create_chunk(
        &self,
        chunk_id: &ChunkId,
        chunk_type: ChunkType,
        chunk_graph: &mut ChunkGraph,
        shared_chunk_names: Vec<String>,
        chunk_group: &Option<ChunkGroup>,
    ) -> (
        Chunk,
        Vec<(ModuleId, Option<ChunkGroup>)>,
        Vec<(ModuleId, Option<ChunkGroup>)>,
    ) {
        crate::mako_profile_function!(&chunk_id.id);
        let mut dynamic_entries = vec![];
        let mut worker_entries = vec![];

        let chunk_id_str = match chunk_group {
            Some(chunk_group) => generate_module_id(&chunk_group.name, &self.context),
            None => chunk_id.generate(&self.context),
        };
        let mut chunk = Chunk::new(chunk_id_str.into(), chunk_type.clone());

        let module_graph = self.context.module_graph.read().unwrap();

        let mut chunk_deps = visit_modules(vec![chunk_id.clone()], None, |head| {
            let mut next_module_ids = vec![];

            for (dep_module_id, dep) in module_graph.get_dependencies(head) {
                match &dep.resolve_type {
                    ResolveType::DynamicImport(chunk_group) => {
                        dynamic_entries.push((dep_module_id.clone(), chunk_group.clone()));
                    }
                    ResolveType::Worker(chunk_group) => {
                        worker_entries.push((dep_module_id.clone(), chunk_group.clone()));
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
                    }
                    _ => {}
                }
            }

            next_module_ids
        });

        // add modules to chunk as dfs order
        while let Some(dep) = chunk_deps.pop() {
            chunk.add_module(dep);
        }

        (chunk, dynamic_entries, worker_entries)
    }

    fn create_update_async_chunks(
        &self,
        async_module_ids: Vec<(ModuleId, Option<ChunkGroup>)>,
        chunk_graph: &mut ChunkGraph,
        shared_chunk_names: Vec<String>,
    ) -> Vec<ChunkId> {
        let mut edges = vec![];
        let mut new_chunks = vec![];

        visit_modules(async_module_ids, None, |head| {
            let (new_chunk, dynamic_dependencies, worker_dependencies) = self.create_chunk(
                &head.0,
                ChunkType::Async,
                chunk_graph,
                shared_chunk_names.clone(),
                &head.1,
            );
            let chunk_id = new_chunk.id.clone();

            // record edges and add chunk to graph
            edges.extend(
                [dynamic_dependencies.clone(), worker_dependencies.clone()]
                    .concat()
                    .iter()
                    .map(|dep| {
                        (
                            chunk_id.clone(),
                            match chunk_graph.get_chunk_for_module(&dep.0) {
                                // ref existing chunk
                                Some(chunk) => chunk.id.clone(),
                                // ref new chunk
                                None => match dep {
                                    (_, Some(chunk_group)) => {
                                        generate_module_id(&chunk_group.name, &self.context).into()
                                    }
                                    (module_id, None) => module_id.generate(&self.context).into(),
                                },
                            },
                        )
                    }),
            );
            chunk_graph.add_chunk(new_chunk);
            new_chunks.push(chunk_id.clone());

            // continue to visit non-existing dynamic dependencies
            dynamic_dependencies
                .into_iter()
                .filter(|dep| chunk_graph.get_chunk_for_module(&dep.0).is_none())
                .collect::<Vec<(ModuleId, Option<ChunkGroup>)>>()
        });

        // add edges
        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }

        new_chunks
    }
}

/*
*  Visit dependencies by right first DFS. The reason for this is that
*  the rightmost and topmost css dependence should have the highest priority.
*  For example, the dependencies graph is:
*
*  ----------
*  index.css
*   -> a.css
*       -> b.css
*           -> c.css
*       -> c.css
*   -> b.css
*       -> c.css
* ----------
* the final dependencies orders in chunk should be:
*
* ----------
* index.css
* b.css
* c.css
* a.css
* ----------
* note that c.css, b.css, c.css after a.css will be deduplicated.
* Notice: the returned Vec must be consumed by revered order.
*/
fn visit_modules<F, T>(mut queue: Vec<T>, visited: Option<HashSet<T>>, mut callback: F) -> Vec<T>
where
    F: FnMut(&T) -> Vec<T>,
    T: Hash + Eq + Clone,
{
    let mut right_first_dfs_ret: Vec<T> = Vec::new();

    let mut visited = visited.unwrap_or_default();

    while let Some(id) = queue.pop() {
        if visited.contains(&id) {
            continue;
        }

        right_first_dfs_ret.push(id.clone());

        visited.insert(id.clone());

        queue.extend(callback(&id));
    }

    right_first_dfs_ret
}
