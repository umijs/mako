use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::vec;

use tracing::debug;

use crate::bfs::{Bfs, NextResult};
use crate::chunk::{Chunk, ChunkType};
use crate::chunk_graph::ChunkGraph;
use crate::compiler::Compiler;
use crate::module::{ModuleId, ResolveType};

impl Compiler {
    // TODO:
    // - 多个 entry 之间的 chunk 共享

    pub fn group_chunk(&self) {
        debug!("group_chunk");

        let visited = Rc::new(RefCell::new(HashSet::new()));
        let mut edges = vec![];
        let module_graph = self.context.module_graph.read().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        chunk_graph.clear();

        let entries = module_graph.get_entry_modules();
        for entry in entries {
            let (chunk, dynamic_dependencies) =
                self.create_chunk(&entry, ChunkType::Entry, &mut chunk_graph, vec![]);
            let chunk_name = chunk.filename();
            visited.borrow_mut().insert(chunk.id.clone());
            edges.extend(
                dynamic_dependencies
                    .clone()
                    .into_iter()
                    .map(|dep| (chunk.id.clone(), dep.generate(&self.context).into())),
            );
            chunk_graph.add_chunk(chunk);

            // handle dynamic dependencies
            let mut bfs = Bfs::new(VecDeque::from(dynamic_dependencies), visited.clone());
            while !bfs.done() {
                match bfs.next_node() {
                    NextResult::Visited => continue,
                    NextResult::First(head) => {
                        let (chunk, dynamic_dependencies) = self.create_chunk(
                            &head,
                            ChunkType::Async,
                            &mut chunk_graph,
                            vec![chunk_name.clone()],
                        );
                        edges.extend(
                            dynamic_dependencies
                                .clone()
                                .into_iter()
                                .map(|dep| (chunk.id.clone(), dep.generate(&self.context).into())),
                        );
                        chunk_graph.add_chunk(chunk);
                        for dep in dynamic_dependencies {
                            bfs.visit(dep);
                        }
                    }
                }
            }
        }

        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }
    }

    fn create_chunk(
        &self,
        entry_module_id: &ModuleId,
        chunk_type: ChunkType,
        chunk_graph: &mut ChunkGraph,
        shared_chunk_names: Vec<String>,
    ) -> (Chunk, Vec<ModuleId>) {
        let mut dynamic_entries = vec![];
        let mut bfs = Bfs::new(VecDeque::from(vec![entry_module_id]), Default::default());

        let chunk_id = entry_module_id.generate(&self.context);
        let mut chunk = Chunk::new(chunk_id.into(), chunk_type);
        let mut visited_modules: Vec<ModuleId> = vec![entry_module_id.clone()];

        let module_graph = self.context.module_graph.read().unwrap();

        while !bfs.done() {
            match bfs.next_node() {
                NextResult::Visited => continue,
                NextResult::First(head) => {
                    let module_already_in_entry = shared_chunk_names.iter().any(|name| {
                        chunk_graph
                            .get_chunk_by_name(name)
                            .unwrap()
                            .has_module(head)
                    });

                    if !module_already_in_entry {
                        let parent_index = visited_modules
                            .iter()
                            .position(|m| m.id == head.id)
                            .unwrap_or(0);
                        let mut normal_deps = vec![];

                        for (dep_module_id, dep) in module_graph.get_dependencies(head) {
                            if dep.resolve_type == ResolveType::DynamicImport {
                                dynamic_entries.push(dep_module_id.clone());
                            } else {
                                bfs.visit(dep_module_id);
                                // collect normal deps for current head
                                normal_deps.push(dep_module_id.clone());
                            }
                        }

                        // insert normal deps before head, so that we can keep the dfs order
                        visited_modules.splice(parent_index..parent_index, normal_deps);
                    }
                }
            }
        }

        // add modules to chunk as dfs order
        for module_id in visited_modules {
            chunk.add_module(module_id);
        }

        (chunk, dynamic_entries)
    }
}
