use nodejs_resolver::Resolver;
use std::{collections::VecDeque, sync::Arc, time::Instant};
use tokio::sync::mpsc::error::TryRecvError;
use tracing::info;

use crate::{
    analyze_deps::analyze_deps,
    ast::build_js_ast,
    compiler::{Compiler, Context},
    load::load,
    module::{Dependency, Module, ModuleAst, ModuleId, ModuleInfo},
    parse::parse,
    resolve::{get_resolver, resolve},
    transform::transform,
};

#[derive(Debug)]
struct Task {
    path: String,
    is_entry: bool,
    parent_module_id: Option<ModuleId>,
    parent_dependency: Option<Dependency>,
}

impl Compiler {
    pub fn build(&self) {
        info!("build");
        let t_build = Instant::now();
        self.build_module_graph();
        let t_build = t_build.elapsed();
        // build chunk map 应该放 generate 阶段
        // 和 chunk 相关的都属于 generate

        info!("build done in {}ms", t_build.as_millis());
    }

    // TODO:
    // - 处理出错（比如找不到模块）的情况，现在会直接挂起
    fn build_module_graph(&self) {
        info!("build module graph");

        let entries = self.get_entries();
        assert!(entries.is_some(), "entry not found");
        let entries = entries.unwrap();
        if entries.len() == 0 {
            panic!("entry not found");
        }

        let resolver = Arc::new(get_resolver(Some(
            self.context.config.resolve.alias.clone(),
        )));
        let mut queue: VecDeque<Task> = VecDeque::new();
        for entry in entries {
            queue.push_back(Task {
                path: entry.to_str().unwrap().to_string(),
                is_entry: true,
                parent_dependency: None,
                parent_module_id: None,
            });
        }

        let (rs, mut rr) = tokio::sync::mpsc::unbounded_channel::<Option<Vec<Task>>>();
        let mut active_task_count: usize = 0;
        tokio::task::block_in_place(move || loop {
            while let Some(task) = queue.pop_front() {
                let resolver = resolver.clone();
                let context = self.context.clone();
                tokio::spawn({
                    active_task_count += 1;
                    let rs = rs.clone();
                    async move {
                        let tasks = Compiler::build_module(context, task, resolver);
                        rs.send(tasks).expect("send task failed");
                    }
                });
            }
            match rr.try_recv() {
                Ok(tasks) => {
                    if let Some(tasks) = tasks {
                        for task in tasks {
                            queue.push_back(task);
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
        resolver: Arc<Resolver>,
    ) -> Option<Vec<Task>> {
        let module_id = ModuleId::new(task.path.clone());

        // load
        let content = load(&task.path, &context);

        // parse
        let (mut ast, cm) = parse(&content, &task.path);

        // transform
        transform(&mut ast, &cm);

        // analyze deps
        // TODO：怎么处理 transform helper 怎么处理？
        // 两个方案，
        // 1. analyze 时跑两边 ast，一遍是原始的，一遍是 transform 之后的
        // 2. build 不处理 helper 模块，在 generate 阶段手动处理
        let deps = analyze_deps(&ast);
        let mut tasks = vec![];
        let mut deps_to_add = vec![];
        for dep in deps {
            let (resolved_path, external) = resolve(&task.path, &dep, &resolver, &context);
            let resolved_path_for_module_create = resolved_path.clone();
            let dep_module_id = ModuleId::new(resolved_path_for_module_create);
            let mut module_graph = context.module_graph.write().unwrap();
            // println!("dep: {:?}", dep);
            // external 的 ast 应该放 generate 阶段处理
            // 因为决定怎么生成代码不是 build 阶段应该感知的事
            if let Some(external) = external {
                // add module for external dependency
                if !module_graph.has_module(&dep_module_id) {
                    let code = format!("module.exports = {};", external);
                    let (cm, ast) = build_js_ast(&resolved_path, code.as_str());
                    module_graph.add_module(Module::new(
                        dep_module_id.clone(),
                        false,
                        Some(ModuleInfo {
                            ast: ModuleAst::Script(ast),
                            cm: Some(cm),
                            path: resolved_path,
                            external: Some(external),
                        }),
                    ));
                }
                deps_to_add.push((dep_module_id, dep));
            } else {
                if module_graph.has_module(&dep_module_id) {
                    deps_to_add.push((dep_module_id, dep));
                } else {
                    // 为啥传 parent_module_id 而不是直接添加到 module_graph？
                    // 因为此时 dep 的 module 还没有被创建好
                    tasks.push(Task {
                        path: resolved_path,
                        is_entry: false,
                        parent_dependency: Some(dep),
                        parent_module_id: Some(module_id.clone()),
                    });
                }
            }
            drop(module_graph);
        }

        // create module and add to module graph
        let info = ModuleInfo {
            ast,
            cm: Some(cm),
            path: task.path.clone(),
            external: None,
        };
        let module = Module::new(module_id.clone(), task.is_entry, Some(info));
        let mut module_graph: std::sync::RwLockWriteGuard<crate::module_graph::ModuleGraph> =
            context.module_graph.write().unwrap();
        // why check?
        // 如果不 check，下述场景会出现重复的 c
        // a -> b_1, a -> b_2, b_1 -> c, b_2 -> c
        if module_graph.has_module(&module_id) {
            module_graph.add_dependency(
                &task.parent_module_id.unwrap(),
                &module_id,
                task.parent_dependency.unwrap(),
            );
            return None;
        } else {
            module_graph.add_module(module);
        }
        // add current module's dependencies
        for (dep_module_id, dep) in deps_to_add {
            module_graph.add_dependency(&module_id, &dep_module_id, dep);
        }
        // add current module
        if task.parent_module_id.is_some() && task.parent_dependency.is_some() {
            module_graph.add_dependency(
                &task.parent_module_id.unwrap(),
                &module_id,
                task.parent_dependency.unwrap(),
            );
        }
        drop(module_graph);

        Some(tasks)
    }

    fn get_entries(&self) -> Option<Vec<std::path::PathBuf>> {
        let root = &self.context.root;
        let entry = &self.context.config.entry;
        if entry.is_empty() {
            let file_paths = vec!["src/index.tsx", "src/index.ts", "index.tsx", "index.ts"];
            for file_path in file_paths {
                let file_path = root.join(file_path);
                if file_path.exists() {
                    return Some(vec![file_path]);
                }
            }
        } else {
            let vals = entry
                .values()
                .map(|v| {
                    let file_path = root.join(v);
                    file_path
                })
                .collect::<Vec<std::path::PathBuf>>();
            return Some(vals);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use petgraph::prelude::EdgeRef;
    use petgraph::visit::IntoEdgeReferences;

    use crate::{compiler, config};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build() {
        let (module_ids, references) = build("test/build/normal");
        // let (module_ids, _) = build("examples/normal");
        assert_eq!(
            module_ids.join(","),
            "bar_1.ts,bar_2.ts,foo.ts,index.ts".to_string()
        );
        assert_eq!(
            references
                .into_iter()
                .map(|(source, target)| {
                    return format!("{} -> {}", source, target);
                })
                .collect::<Vec<String>>()
                .join(","),
            "bar_1.ts -> foo.ts,bar_2.ts -> foo.ts,index.ts -> bar_1.ts,index.ts -> bar_2.ts"
                .to_string()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build_css() {
        let (module_ids, references) = build("test/build/css");
        assert_eq!(
            module_ids.join(","),
            "foo.css,index.css,index.ts".to_string()
        );
        assert_eq!(
            references
                .into_iter()
                .map(|(source, target)| {
                    return format!("{} -> {}", source, target);
                })
                .collect::<Vec<String>>()
                .join(","),
            "index.css -> foo.css,index.ts -> index.css".to_string()
        );
    }

    fn build(base: &str) -> (Vec<String>, Vec<(String, String)>) {
        let current_dir = std::env::current_dir().unwrap();
        // let fixtures = current_dir.join("test/build");
        let pnpm_dir = current_dir.join("node_modules/.pnpm");
        let root = current_dir.join(base);
        let config = config::Config::new(&root).unwrap();
        let compiler = compiler::Compiler::new(config, root.clone());
        compiler.build();
        let module_graph = compiler.context.module_graph.read().unwrap();
        let mut module_ids: Vec<String> = module_graph
            .graph
            .node_weights()
            .into_iter()
            .map(|module| {
                module
                    .id
                    .id
                    .to_string()
                    .replace(format!("{}/", root.to_str().unwrap()).as_str(), "")
                    .replace(pnpm_dir.to_str().unwrap(), "")
            })
            .collect();
        module_ids.sort_by_key(|module_id| module_id.to_string());
        // module_graph.fmt();
        let mut references: Vec<(String, String)> = module_graph
            .graph
            .edge_references()
            .into_iter()
            .map(|edge| {
                let source = &module_graph.graph[edge.source()].id.id;
                let target = &module_graph.graph[edge.target()].id.id;
                (
                    source
                        .to_string()
                        .replace(format!("{}/", root.to_str().unwrap()).as_str(), "")
                        .replace(pnpm_dir.to_str().unwrap(), ""),
                    target
                        .to_string()
                        .replace(format!("{}/", root.to_str().unwrap()).as_str(), "")
                        .replace(pnpm_dir.to_str().unwrap(), ""),
                )
            })
            .collect();
        references.sort_by_key(|(source, target)| format!("{} -> {}", source, target).to_string());

        println!("module_ids:");
        for module_id in &module_ids {
            println!("  - {:?}", module_id);
        }
        println!("references:");
        for (source, target) in &references {
            println!("  - {} -> {}", source, target);
        }

        (module_ids, references)
    }
}
