use crate::chunk::{Chunk, ChunkType};
use crate::compiler::Compiler;
use crate::module::ModuleId;
use crate::module_graph::{ModuleGraph, ResolveType};
use crate::utils::bfs::{Bfs, NextResult};
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

impl Compiler {
    // 通过 BFS 搜索从入口模块进入后的所有依赖，直到遇到 DynamicImport 为止，作为一个 chunk
    // TODO: 后续可增加 common-chunk 算法等
    pub fn grouping_chunks(&mut self) {
        let visited = Rc::new(RefCell::new(HashSet::new()));
        let mut module_graph = self.context.module_graph.write().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        let mut edges = vec![];
        let entries_modules = module_graph.get_entry_modules();
        for entry_id in entries_modules {
            // 处理入口 chunk
            let (chunk, dynamic_dependencies) = Self::create_chunk_by_entry_module_id(
                &mut module_graph,
                &entry_id,
                ChunkType::Entry,
            );
            visited.borrow_mut().insert(entry_id.clone());

            edges.extend(
                dynamic_dependencies
                    .clone()
                    .into_iter()
                    .map(|dep| (chunk.id.clone(), dep)),
            );

            chunk_graph.add_chunk(chunk);

            // 处理 dynamic import 部分的chunk
            let mut bfs = Bfs::new(VecDeque::from(dynamic_dependencies), visited.clone());
            while !bfs.done() {
                match bfs.next_node() {
                    NextResult::Visited => continue,
                    NextResult::First(head) => {
                        let (chunk, dynamic_dependencies) = Self::create_chunk_by_entry_module_id(
                            &mut module_graph,
                            &head,
                            ChunkType::Async,
                        );

                        edges.extend(
                            dynamic_dependencies
                                .clone()
                                .into_iter()
                                .map(|dep| (chunk.id.clone(), dep)),
                        );

                        chunk_graph.add_chunk(chunk);
                        for dep_module_id in &dynamic_dependencies {
                            bfs.visit(dep_module_id.clone());
                        }
                    }
                }
            }
        }

        for (from, to) in &edges {
            chunk_graph.add_edge(from, to);
        }
    }

    fn create_chunk_by_entry_module_id(
        module_graph: &mut ModuleGraph,
        entry_module_id: &ModuleId,
        chunk_type: ChunkType,
    ) -> (Chunk, Vec<ModuleId>) {
        let mut dynamic_entries = vec![];
        let mut bfs = Bfs::new(VecDeque::from(vec![entry_module_id]), Default::default());
        let mut chunk = Chunk::new(entry_module_id.clone(), chunk_type);

        while !bfs.done() {
            match bfs.next_node() {
                NextResult::Visited => continue,
                NextResult::First(head) => {
                    // bind module to chunk
                    chunk.add_module(head.clone());

                    for (dep_module_id, dep) in module_graph.get_dependencies(head) {
                        if dep.resolve_type == ResolveType::DynamicImport {
                            dynamic_entries.push(dep_module_id.clone());
                        } else {
                            bfs.visit(dep_module_id);
                        }
                    }
                }
            }
        }

        // bind chunk to module
        chunk.get_modules().into_iter().for_each(|module_id| {
            module_graph
                .get_module_mut(module_id)
                .unwrap()
                .chunks
                .insert(chunk.id.clone());
        });
        (chunk, dynamic_entries)
    }
}
