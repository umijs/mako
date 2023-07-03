use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hasher;
use std::path::Path;
use std::rc::Rc;
use std::vec;

use anyhow::Result;
use cached::proc_macro::cached;
use indexmap::IndexSet;
use serde::Deserialize;
use tracing::debug;
use twox_hash::XxHash64;

use crate::bfs::{Bfs, NextResult};
use crate::chunk::{Chunk, ChunkId, ChunkType};
use crate::chunk_graph::ChunkGraph;
use crate::compiler::Compiler;
use crate::config::{CodeSplittingStrategy, Mode};
use crate::module::{ModuleId, ResolveType};

impl Compiler {
    // TODO:
    // - 多个 entry 之间的 chunk 共享
    // - 支持各种 chunk 拆分策略，比如把所有 node_modules 下的包按 package name 拆

    pub fn group_chunk(&self) {
        self.group_main_chunk();

        match self.context.config.code_splitting {
            CodeSplittingStrategy::BigVendor => {
                self.group_big_vendor_chunk();
            }
            CodeSplittingStrategy::DepPerChunk => {
                self.group_dep_per_chunk();
            }
            CodeSplittingStrategy::None => {
                // do nothing, use the main chunk only
            }
        }
    }

    pub fn group_dep_per_chunk(&self) {
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();

        let mut entries = chunk_graph.mut_chunks();

        let mut pkg_modules: HashMap<String, IndexSet<ModuleId>> = HashMap::new();
        let mut pkg_chunk_dependant: HashMap<String, IndexSet<ChunkId>> = HashMap::new();

        // keep module order stable for each splitting
        entries.sort_by_key(|c| c.id.id.clone());

        for chunk in entries {
            let mut to_remove = vec![];
            for m_id in chunk.get_modules().iter().collect::<Vec<&ModuleId>>() {
                let pkg_name = match self.context.config.mode {
                    Mode::Development => read_pkg_name_from_pkg_json(&m_id.id),
                    Mode::Production => guess_pkg_name_from_path(&m_id.id),
                };

                match pkg_name {
                    None => continue,
                    Some(pkg_name) => {
                        pkg_modules
                            .entry(pkg_name.clone())
                            .or_default()
                            .insert(m_id.clone());

                        to_remove.push(m_id.clone());

                        pkg_chunk_dependant
                            .entry(pkg_name)
                            .or_default()
                            .insert(chunk.id.clone());
                    }
                }
            }

            for m_id in to_remove {
                chunk.remove_module(&m_id);
            }
        }

        for (pkg_name, modules) in pkg_modules {
            let mut chunk = Chunk::new(pkg_name.clone().into(), ChunkType::Sync);

            for m_id in modules {
                chunk.add_module(m_id);
            }
            chunk_graph.add_chunk(chunk);

            let dependant_chunks = pkg_chunk_dependant.get(&pkg_name).unwrap();

            for dep_chunk in dependant_chunks {
                chunk_graph.add_edge(dep_chunk, &pkg_name.clone().into());
            }
        }
    }

    pub fn group_main_chunk(&self) {
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

    fn group_big_vendor_chunk(&self) {
        // big vendors chunk policy
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        let mut chunks = chunk_graph.mut_chunks();
        let mut big_vendor_chunk = Chunk::new("all_vendors".into(), ChunkType::Sync);

        let mut entries = Vec::new();

        // keep module order stable for each splitting
        chunks.sort_by_key(|c| c.id.id.clone());

        for c in chunks {
            let mut vendors_to_move = IndexSet::new();

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

#[derive(Debug, Deserialize, Clone)]
struct PackageJson {
    name: String,
    version: String,
}

fn guess_pkg_name_from_path<T: AsRef<str>>(path: T) -> Option<String> {
    let path = path.as_ref();

    let (node_modules_root, name_part) = root_and_pkg_name(path)?;

    let pkg_path = Path::new(&node_modules_root)
        .join(name_part)
        .join("package.json");

    match load_pkg_json(pkg_path.to_str()?) {
        Ok(pkg) => Some(format!("{}@{}", pkg.name, pkg.version)),
        Err(_) => None,
    }
}

fn read_pkg_name_from_pkg_json<T: AsRef<str>>(path: T) -> Option<String> {
    let path = path.as_ref();

    let (node_modules_root, name_part) = root_and_pkg_name(path)?;
    let hash = hash_path(&node_modules_root);

    Some(format!("{}@{:8x}", name_part, hash))
}

#[cached(key = "String", convert = r#"{ String::from(p) }"#)]
fn hash_path(p: &str) -> u64 {
    let mut hasher: XxHash64 = Default::default();
    hasher.write(p.as_bytes());
    hasher.finish()
}

fn guess_pkg_name(name: String) -> Option<String> {
    let parts: Vec<&str> = name.split('/').collect();

    if parts.is_empty() {
        return None;
    }

    if parts[0].starts_with('@') && parts.len() > 1 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        Some(parts[0].to_string())
    }
}

#[cached(result = true, key = "String", convert = r#"{ String::from(p) }"#)]
fn load_pkg_json(p: &str) -> Result<PackageJson> {
    let file = std::fs::File::open(p)?;
    let reader = std::io::BufReader::new(file);
    let pkg_json: PackageJson = serde_json::from_reader(reader)?;
    Ok(pkg_json)
}

fn root_and_pkg_name(path: &str) -> Option<(String, String)> {
    let node_modules_root = path.rfind("/node_modules/").map(|idx| &path[0..idx + 14])?;

    let right_parts = path.rfind("/node_modules/").map(|idx| &path[idx + 14..])?;
    let name_part = guess_pkg_name(right_parts.to_string())?;
    Some((node_modules_root.to_string(), name_part))
}
