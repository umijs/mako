use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use tracing::info;

use crate::bfs::{Bfs, NextResult};
use crate::chunk::{Chunk, ChunkType};
use crate::compiler::Compiler;
use crate::module::{ModuleId, ResolveType};

impl Compiler {
    // TODO:
    // - 多个 entry 之间的 chunk 共享
    // - 支持各种 chunk 拆分策略，比如把所有 node_modules 下的包按 package name 拆

    pub fn group_chunk(&self) {
        self.group_main_chunk();

        self.group_big_vendor_chunk();
    }

    pub fn group_main_chunk(&self) {
        info!("group_chunk");

        let visited = Rc::new(RefCell::new(HashSet::new()));
        let mut edges = vec![];
        let module_graph = self.context.module_graph.read().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        chunk_graph.clear();

        let entries = module_graph.get_entry_modules();
        for entry in entries {
            let (chunk, dynamic_dependencies) = self.create_chunk(&entry, ChunkType::Entry);
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
                        let (chunk, dynamic_dependencies) =
                            self.create_chunk(&head, ChunkType::Async);
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

    fn group_big_vendor_chunk(&self) {
        // big vendors chunk policy
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        let chunks = chunk_graph.mut_chunks();
        let mut big_vendor_chunk = Chunk::new("all_vendors".into(), ChunkType::Sync);

        let mut entries = Vec::new();

        for c in chunks {
            let mut vendors_to_move = HashSet::new();

            for m in c
                .mut_modules()
                .iter()
                .filter(|&m| m.id.contains("node_modules"))
            {
                vendors_to_move.insert(m.clone());
                big_vendor_chunk.add_module(m.clone())
            }

            for m in &vendors_to_move {
                c.remove_module(m);
            }

            if matches!(c.chunk_type, ChunkType::Entry) {
                entries.push(c.id.clone());
            }
        }

        let to_chunk = big_vendor_chunk.id.clone();
        chunk_graph.add_chunk(big_vendor_chunk);
        for entry in entries {
            chunk_graph.add_edge(&entry, &to_chunk);
        }
    }

    fn create_chunk(
        &self,
        entry_module_id: &ModuleId,
        chunk_type: ChunkType,
    ) -> (Chunk, Vec<ModuleId>) {
        let mut dynamic_entries = vec![];
        let mut bfs = Bfs::new(VecDeque::from(vec![entry_module_id]), Default::default());

        let chunk_id = entry_module_id.generate(&self.context);
        let mut chunk = Chunk::new(chunk_id.into(), chunk_type);
        chunk.add_module(entry_module_id.clone());

        let module_graph = self.context.module_graph.read().unwrap();
        while !bfs.done() {
            match bfs.next_node() {
                NextResult::Visited => continue,
                NextResult::First(head) => {
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

        // TODO:
        // 这里我删除了 bind chunk to module 的逻辑
        // 因为还没有看到在哪里会用到

        (chunk, dynamic_entries)
    }
}
