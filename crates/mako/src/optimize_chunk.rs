use regex::Regex;

use crate::chunk::{Chunk, ChunkType};
use crate::compiler::Compiler;

pub enum OptimizeAllowChunks {
    // All,
    Entry,
    Async,
}

pub struct OptimizeChunkOptions {
    pub min_size: usize,
    pub groups: Vec<OptimizeChunkGroup>,
}

pub struct OptimizeChunkGroup {
    pub name: String,
    pub allow_chunks: OptimizeAllowChunks,
    pub min_chunks: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub test: Option<Regex>,
    pub priority: Option<i8>,
}

impl Compiler {
    pub fn optimize_chunk(&self) {
        if let Some(optimize_options) = self.get_optimize_chunk_options() {
            // stage: deasync
            self.merge_minimal_async_chunks(&optimize_options);
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

        for (chunk_id, entry_chunk_id, chunk_modules) in async_to_entry.clone() {
            let entry_chunk = chunk_graph.mut_chunk(&entry_chunk_id).unwrap();

            // merge modules to entry chunk
            for m in chunk_modules {
                entry_chunk.add_module(m);
            }

            // remove original async chunks
            chunk_graph.remove_chunk(&chunk_id);
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
                        priority: None,
                    },
                    OptimizeChunkGroup {
                        name: "vendors_dynamic".to_string(),
                        allow_chunks: OptimizeAllowChunks::Async,
                        min_chunks: 1,
                        min_size: 20000,
                        max_size: 5000000,
                        test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                        priority: None,
                    },
                ],
            }),
            _ => None,
        }
    }
}
