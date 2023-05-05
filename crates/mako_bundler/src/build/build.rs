use maplit::hashset;
use nodejs_resolver::{Options, Resolver};
use std::sync::Arc;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

use crate::utils::bfs::{Bfs, NextResult};
use crate::{
    chunk::ChunkType,
    compiler::Compiler,
    config::get_first_entry_value,
    module::{Module, ModuleId, ModuleInfo},
    module_graph::Dependency,
};

use super::{
    analyze_deps::{analyze_deps, AnalyzeDepsParam},
    load::{load, LoadParam},
    parse::{parse, ParseParam},
    resolve::{resolve, ResolveParam},
    transform::transform::{transform, TransformParam},
};

pub struct BuildParam {
    pub files: Option<HashMap<String, String>>,
}

struct Task {
    pub path: String,
    pub parent_module_id: Option<ModuleId>,
    pub parent_dependecy: Option<Dependency>,
}

impl Compiler {
    pub fn build(&mut self, build_param: &BuildParam) {
        let cwd = &self.context.config.root;
        let entry_point = cwd
            .join(get_first_entry_value(&self.context.config.entry).unwrap())
            .to_string_lossy()
            .to_string();

        // build
        self.build_module_graph(entry_point, build_param);

        // chunks
        self.grouping_chunks();
    }

    fn build_module_graph(&mut self, entry_point: String, build_param: &BuildParam) {
        let mut queue: Vec<Task> = vec![Task {
            path: entry_point.clone(),
            parent_module_id: None,
            parent_dependecy: None,
        }];

        let resolver: Resolver = Resolver::new(Options {
            extensions: vec![
                ".js".to_string(),
                ".jsx".to_string(),
                ".ts".to_string(),
                ".tsx".to_string(),
                ".mjs".to_string(),
                ".cjs".to_string(),
            ],
            condition_names: hashset! {
                "node".to_string(),
                "require".to_string(),
                "import".to_string(),
                "browser".to_string(),
                "default".to_string()
            },
            external_cache: Some(Arc::new(Default::default())),
            ..Default::default()
        });

        while !queue.is_empty() {
            let task = queue.pop().unwrap();
            let path_str = task.path.as_str();

            let module_id = ModuleId::new(path_str);
            let is_entry = path_str == entry_point;

            // check if module is already in the graph
            if self.context.module_graph.has_module(&module_id) {
                self.bind_dependency(&task, &module_id);
                continue;
            }

            // load
            let load_param = LoadParam {
                path: path_str,
                files: build_param.files.as_ref(),
            };
            let load_result = load(&load_param, &mut self.context);

            // parse
            let parse_param = ParseParam {
                path: path_str,
                content: load_result.content,
                content_type: load_result.content_type,
            };
            let parse_result = parse(&parse_param, &self.context);

            // transform
            let transform_param = TransformParam {
                path: path_str,
                ast: &parse_result.ast,
                cm: &parse_result.cm,
            };
            let transform_result = transform(&transform_param, &self.context);

            // add current module to module graph
            let info = ModuleInfo {
                path: task.path.clone(),
                is_external: false,
                external_name: None,
                is_entry,
                original_cm: Some(parse_result.cm),
                original_ast: transform_result.ast.clone(),
            };
            let module = Module::new(module_id.clone(), info);

            // setup entry module
            if module.info.is_entry {
                self.context.module_graph.add_entry_module(module);
            } else {
                self.context.module_graph.add_module(module);
            }

            // handle dependency bind
            self.bind_dependency(&task, &module_id);

            // analyze deps
            // if info.original_ast matches ModuleAst::Script
            let analyze_deps_param = AnalyzeDepsParam {
                path: path_str,
                ast: &parse_result.ast,
                transform_ast: &transform_result.ast,
            };
            let analyze_deps_result = analyze_deps(&analyze_deps_param, &self.context);

            // resolve
            for d in &analyze_deps_result.dependencies {
                let resolve_param = ResolveParam {
                    path: path_str,
                    dependency: &d.source,
                    files: build_param.files.as_ref(),
                };
                let resolve_result = resolve(&resolve_param, &self.context, &resolver);
                println!(
                    "> resolve {} from {} -> {}",
                    &d.source, path_str, resolve_result.path
                );
                if resolve_result.is_external {
                    let external_name = resolve_result.external_name.unwrap();
                    let info = ModuleInfo {
                        path: resolve_result.path.clone(),
                        is_external: resolve_result.is_external,
                        external_name: Some(external_name),
                        is_entry: false,
                        original_cm: None,
                        original_ast: crate::module::ModuleAst::None,
                    };
                    let extrnal_module_id = ModuleId::new(&resolve_result.path);
                    let extranl_module = Module::new(extrnal_module_id.clone(), info);
                    self.context.module_graph.add_module(extranl_module);
                    self.context.module_graph.add_dependency(
                        &module_id,
                        &extrnal_module_id,
                        d.clone(),
                    )
                } else {
                    queue.push(Task {
                        parent_module_id: Some(module_id.clone()),
                        path: resolve_result.path,
                        parent_dependecy: Some(d.clone()),
                    });
                }
            }
        }
    }

    fn bind_dependency(&mut self, task: &Task, module_id: &ModuleId) {
        if let Some(parent_module_id) = &task.parent_module_id {
            let parent_dependency = task
                .parent_dependecy
                .as_ref()
                .expect("parent dependency is required for parent_module_id");
            self.context.module_graph.add_dependency(
                parent_module_id,
                module_id,
                parent_dependency.clone(),
            )
        }
    }

    // 通过 BFS 搜索从入口模块进入后的所有依赖，直到遇到 DynamicImport 为止，作为一个 chunk
    // TODO: 后续可增加 common-chunk 算法等
    fn grouping_chunks(&mut self) {
        let visited = Rc::new(RefCell::new(HashSet::new()));
        let chunk_graph = &mut self.context.chunk_graph;
        let mut edges = vec![];
        let entries_modules = self.context.module_graph.get_entry_modules();
        for entry_id in entries_modules {
            // 处理入口 chunk
            let (chunk, dynamic_dependencies) = self
                .context
                .module_graph
                .create_chunk_by_entry_module_id(&entry_id, ChunkType::Entry);
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
                        let (chunk, dynamic_dependencies) = self
                            .context
                            .module_graph
                            .create_chunk_by_entry_module_id(&head, ChunkType::Async);

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
}
