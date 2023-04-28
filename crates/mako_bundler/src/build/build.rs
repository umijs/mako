use maplit::hashset;
use nodejs_resolver::{Options, Resolver};

use std::sync::Arc;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};
use tokio::sync::mpsc::error::TryRecvError;

use crate::context::Context;

use crate::utils::bfs::{Bfs, NextResult};
use crate::{
    chunk::ChunkType,
    compiler::Compiler,
    config::get_first_entry_value,
    module::{Module, ModuleAst, ModuleId, ModuleInfo},
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

#[derive(Debug)]
struct Task {
    pub path: String,
    pub parent_module_id: Option<ModuleId>,
    pub parent_dependency: Option<Dependency>,
}

#[derive(Debug)]
enum BuildModuleGraphResult {
    Skip,
    Done,
    Next(Vec<Task>),
}

impl Compiler {
    pub fn build(&mut self, build_param: &'static BuildParam) {
        let cwd = &self.context.config.root;
        let entry_point = cwd
            .join(get_first_entry_value(&self.context.config.entry).unwrap())
            .to_string_lossy()
            .to_string();

        // build
        self.build_module_graph_threaded(entry_point, build_param);

        self.grouping_chunks();
    }

    fn build_module_graph_threaded(
        &mut self,
        entry_point: String,
        _build_param: &'static BuildParam,
    ) {
        let resolver = Arc::new(Resolver::new(Options {
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
        }));

        let mut queue: VecDeque<Task> = VecDeque::new();
        queue.push_back(Task {
            path: entry_point.clone(),
            parent_module_id: None,
            parent_dependency: None,
        });

        let (result_sender, mut result_receiver) =
            tokio::sync::mpsc::unbounded_channel::<BuildModuleGraphResult>();
        let mut active_task_count = 0usize;
        tokio::task::block_in_place(move || loop {
            while let Some(task) = queue.pop_front() {
                let ctx = self.context.clone();
                let ep = entry_point.clone();
                let res = resolver.clone();
                tokio::spawn({
                    active_task_count += 1;
                    let sender = result_sender.clone();
                    async move {
                        let result = Self::build_module(ctx, task, ep, _build_param, res);
                        sender.send(result).expect("Failed to send build result");
                    }
                });
            }
            match result_receiver.try_recv() {
                Ok(result) => {
                    match result {
                        BuildModuleGraphResult::Skip => {}
                        BuildModuleGraphResult::Done => {}
                        BuildModuleGraphResult::Next(tasks) => {
                            for task in tasks {
                                queue.push_back(task);
                            }
                        }
                    }
                    active_task_count -= 1;
                }
                Err(TryRecvError::Empty) => {
                    if active_task_count == 0 {
                        break;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        });
    }

    fn build_module(
        context: Arc<Context>,
        task: Task,
        entry_point: String,
        build_param: &BuildParam,
        resolver: Arc<Resolver>,
    ) -> BuildModuleGraphResult {
        let path_str = task.path.as_str();
        //
        let module_id = ModuleId::new(path_str);
        let is_entry = path_str == entry_point;

        let module_graph_r = context.module_graph.read().unwrap();
        // check if module is already in the graph
        if module_graph_r.has_module(&module_id) {
            drop(module_graph_r);
            Self::bind_dependency(&context, &task, &module_id);
            return BuildModuleGraphResult::Skip;
        }
        drop(module_graph_r);
        // load
        let load_param = LoadParam {
            path: path_str,
            files: build_param.files.as_ref(),
        };
        let load_result = load(&load_param, &context);

        // parse
        let parse_param = ParseParam {
            path: path_str,
            content: load_result.content,
        };
        let parse_result = parse(&parse_param, &context);

        // transform
        let transform_param = TransformParam {
            path: path_str,
            ast: &ModuleAst::Script(parse_result.ast.clone()),
            cm: &parse_result.cm,
        };
        let transform_result = transform(&transform_param, &context);

        // add current module to module graph
        let info = ModuleInfo {
            path: task.path.clone(),
            is_external: false,
            external_name: None,
            is_entry,
            original_cm: Some(parse_result.cm),
            original_ast: ModuleAst::Script(transform_result.ast),
        };
        let module = Module::new(module_id.clone(), info);

        // setup entry module
        let mut module_graph_w = context.module_graph.write().unwrap();
        if module.info.is_entry {
            module_graph_w.add_entry_module(module);
        } else {
            module_graph_w.add_module(module);
        }
        drop(module_graph_w);

        // handle dependency bind
        Self::bind_dependency(&context, &task, &module_id);

        // analyze deps
        let analyze_deps_param = AnalyzeDepsParam {
            path: path_str,
            ast: &parse_result.ast,
        };
        let analyze_deps_result = analyze_deps(&analyze_deps_param, &context);
        let mut tasks = vec![];
        // resolve
        for d in &analyze_deps_result.dependencies {
            let resolve_param = ResolveParam {
                path: path_str,
                dependency: &d.source,
                files: None,
            };
            let resolve_result = resolve(&resolve_param, &context, &resolver);
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
                let external_module_id = ModuleId::new(&resolve_result.path);
                let external_module = Module::new(external_module_id.clone(), info);
                let mut module_graph_w = context.module_graph.write().unwrap();
                module_graph_w.add_module(external_module);
                module_graph_w.add_dependency(&module_id, &external_module_id, d.clone());
                drop(module_graph_w);
            } else {
                tasks.push(Task {
                    parent_module_id: Some(module_id.clone()),
                    path: resolve_result.path,
                    parent_dependency: Some(d.clone()),
                });
            }
        }

        if tasks.is_empty() {
            return BuildModuleGraphResult::Done;
        }
        BuildModuleGraphResult::Next(tasks)
    }

    fn bind_dependency(context: &Arc<Context>, task: &Task, module_id: &ModuleId) {
        if let Some(parent_module_id) = &task.parent_module_id {
            let parent_dependency = task
                .parent_dependency
                .as_ref()
                .expect("parent dependency is required for parent_module_id");

            let mut module_graph = context.module_graph.write().unwrap();
            module_graph.add_dependency(parent_module_id, module_id, parent_dependency.clone());
            drop(module_graph);
        }
    }

    // 通过 BFS 搜索从入口模块进入后的所有依赖，直到遇到 DynamicImport 为止，作为一个 chunk
    // TODO: 后续可增加 common-chunk 算法等
    fn grouping_chunks(&mut self) {
        let visited = Rc::new(RefCell::new(HashSet::new()));
        let mut module_graph = self.context.module_graph.write().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();
        let mut edges = vec![];
        let entries_modules = module_graph.get_entry_modules();
        for entry_id in entries_modules {
            // 处理入口 chunk
            let (chunk, dynamic_dependencies) =
                module_graph.create_chunk_by_entry_module_id(&entry_id, ChunkType::Entry);
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
                        let (chunk, dynamic_dependencies) =
                            module_graph.create_chunk_by_entry_module_id(&head, ChunkType::Async);

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
