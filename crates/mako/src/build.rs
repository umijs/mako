use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use nodejs_resolver::Resolver;
use tokio::sync::mpsc::error::TryRecvError;
use tracing::info;

use crate::analyze_deps::analyze_deps;
use crate::ast::build_js_ast;
use crate::compiler::{Compiler, Context};
use crate::config::Config;
use crate::load::load;
use crate::module::{Dependency, Module, ModuleAst, ModuleId, ModuleInfo};
use crate::parse::parse;
use crate::resolve::{get_resolver, resolve};
use crate::transform::transform;

#[derive(Debug)]
pub struct Task {
    pub path: String,
    pub is_entry: bool,
}

pub type ModuleDeps = Vec<(String, Option<String>, Dependency)>;

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

    fn build_module_graph(&self) {
        info!("build module graph");

        let entries =
            get_entries(&self.context.root, &self.context.config).expect("entry not found");
        if entries.is_empty() {
            panic!("entry not found");
        }

        let resolver = Arc::new(get_resolver(&self.context.config));
        let mut queue: VecDeque<Task> = VecDeque::new();
        for entry in entries {
            queue.push_back(Task {
                path: entry.to_str().unwrap().to_string(),
                is_entry: true,
            });
        }

        self.build_module_graph_by_task_queue(&mut queue, resolver);
    }

    pub fn build_module_graph_by_task_queue(
        &self,
        queue: &mut VecDeque<Task>,
        resolver: Arc<Resolver>,
    ) -> HashSet<ModuleId> {
        let (rs, mut rr) =
            tokio::sync::mpsc::unbounded_channel::<Result<(Module, ModuleDeps, Task)>>();
        let mut active_task_count: usize = 0;
        let mut t_main_thread: usize = 0;
        let mut module_count: usize = 0;
        let mut added_module_ids = HashSet::new();
        tokio::task::block_in_place(|| loop {
            let mut module_graph = self.context.module_graph.write().unwrap();
            while let Some(task) = queue.pop_front() {
                let resolver = resolver.clone();
                let context = self.context.clone();
                tokio::spawn({
                    active_task_count += 1;
                    module_count += 1;
                    let rs = rs.clone();
                    async move {
                        let ret = Compiler::build_module(context, task, resolver);
                        rs.send(ret).expect("send task failed");
                    }
                });
            }
            match rr.try_recv() {
                Ok(ret) => {
                    let (module, deps, task) = match ret {
                        Ok(ret) => ret,
                        Err(err) => {
                            panic!("build module failed: {:?}", err);
                        }
                    };
                    let t = Instant::now();

                    // current module
                    let module_id = module.id.clone();
                    // 只有处理 entry 时，module 会不存在于 module_graph 里
                    // 否则，module 会存在于 module_graph 里，只需要补充 info 信息即可
                    if task.is_entry {
                        added_module_ids.insert(module_id.clone());
                        module_graph.add_module(module);
                    } else {
                        let m = module_graph.get_module_mut(&module_id).unwrap();
                        m.add_info(module.info);
                    }

                    // deps
                    deps.iter().for_each(|dep| {
                        let resolved_path = dep.0.clone();
                        let is_external = dep.1.is_some();
                        let dep_module_id = ModuleId::new(resolved_path.clone());
                        let dependency = dep.2.clone();

                        if !module_graph.has_module(&dep_module_id) {
                            let module = self.create_module(
                                dep.1.clone(),
                                resolved_path.clone(),
                                &dep_module_id,
                            );
                            match module {
                                Ok(module) => {
                                    if !is_external {
                                        queue.push_back(Task {
                                            path: resolved_path,
                                            // parent_module_id: None,
                                            is_entry: false,
                                        });
                                    }
                                    // 拿到依赖之后需要直接添加 module 到 module_graph 里，不能等依赖 build 完再添加
                                    // 由于是异步处理各个模块，后者会导致大量重复任务的 build_module 任务（3 倍左右）
                                    added_module_ids.insert(module.id.clone());
                                    module_graph.add_module(module);
                                }
                                Err(err) => {
                                    panic!("create module failed: {:?}", err);
                                }
                            }
                        }
                        module_graph.add_dependency(&module_id, &dep_module_id, dependency);
                    });
                    active_task_count -= 1;
                    let t = t.elapsed();
                    t_main_thread += t.as_micros() as usize;
                }
                Err(TryRecvError::Empty) => {
                    if active_task_count == 0 {
                        info!("build time in main thread: {}ms", t_main_thread / 1000);
                        info!("module count: {}", module_count);
                        break;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        });
        added_module_ids
    }

    pub fn create_module(
        &self,
        external: Option<String>,
        resolved_path: String,
        dep_module_id: &ModuleId,
    ) -> Result<Module> {
        let module = match external {
            Some(external) => {
                // support empty external
                let code = if external.is_empty() {
                    "module.exports = {};".to_string()
                } else {
                    format!("module.exports = {};", external)
                };
                let ast = build_js_ast(
                    format!("external_{}", &resolved_path).as_str(),
                    code.as_str(),
                    &self.context,
                )?;

                Module::new(
                    dep_module_id.clone(),
                    false,
                    Some(ModuleInfo {
                        ast: ModuleAst::Script(ast),
                        path: resolved_path,
                        external: Some(external),
                    }),
                )
            }
            None => Module::new(dep_module_id.clone(), false, None),
        };
        Ok(module)
    }

    pub fn build_module(
        context: Arc<Context>,
        task: Task,
        resolver: Arc<Resolver>,
    ) -> Result<(Module, ModuleDeps, Task)> {
        let mut dependencies = Vec::new();
        let module_id = ModuleId::new(task.path.clone());

        // load
        let content = load(&task.path, task.is_entry, &context)?;

        // parse
        let mut ast = parse(&content, &task.path, &context)?;

        // transform & resolve
        // TODO: 支持同时有多个 resolve error
        let mut dep_resolve_err = None;
        transform(&mut ast, &context, &task, &mut |ast| {
            let deps = analyze_deps(ast);
            // resolve
            for dep in deps.iter() {
                let ret = resolve(&task.path, dep, &resolver, &context);
                match ret {
                    Ok((x, y)) => {
                        dependencies.push((x, y, dep.clone()));
                    }
                    Err(err) => {
                        dep_resolve_err = Some(err);
                        return dependencies.clone();
                    }
                }
            }
            dependencies.clone()
        })?;
        if let Some(e) = dep_resolve_err {
            return Err(e);
        }

        let info = ModuleInfo {
            ast,
            path: task.path.clone(),
            external: None,
        };
        let module = Module::new(module_id, task.is_entry, Some(info));

        Ok((module, dependencies, task))
    }
}

fn get_entries(root: &Path, config: &Config) -> Option<Vec<std::path::PathBuf>> {
    let entry = &config.entry;
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
            .map(|v| root.join(v))
            .collect::<Vec<std::path::PathBuf>>();
        return Some(vals);
    }
    None
}

#[cfg(test)]
mod tests {
    use petgraph::prelude::EdgeRef;
    use petgraph::visit::IntoEdgeReferences;

    use crate::compiler;
    use crate::config::Config;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build_normal() {
        let (module_ids, references) = build("test/build/normal");
        assert_eq!(
            module_ids.join(","),
            "bar_1.ts,bar_2.ts,foo.ts,hoo,index.ts".to_string()
        );
        assert_eq!(
            references
                .into_iter()
                .map(|(source, target)| { format!("{} -> {}", source, target) })
                .collect::<Vec<String>>()
                .join(","),
            "bar_1.ts -> foo.ts,bar_2.ts -> foo.ts,index.ts -> bar_1.ts,index.ts -> bar_2.ts,index.ts -> hoo"
                .to_string()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build_config_entry() {
        let (module_ids, _references) = build("test/build/config-entry");
        assert_eq!(module_ids.join(","), "bar.ts,foo.ts".to_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic]
    async fn test_build_panic_resolve() {
        build("test/build/panic-resolve");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build_css() {
        let (module_ids, references) = build("test/build/css");
        assert_eq!(
            module_ids.join(","),
            "foo.css,index.css,index.ts,umi-logo.png".to_string()
        );
        assert_eq!(
            references
                .into_iter()
                .map(|(source, target)| { format!("{} -> {}", source, target) })
                .collect::<Vec<String>>()
                .join(","),
            "index.css -> foo.css,index.css -> umi-logo.png,index.ts -> index.css".to_string()
        );
    }

    fn build(base: &str) -> (Vec<String>, Vec<(String, String)>) {
        let current_dir = std::env::current_dir().unwrap();
        let pnpm_dir = current_dir.join("node_modules/.pnpm");
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        let compiler = compiler::Compiler::new(config, root.clone());
        compiler.build();
        let module_graph = compiler.context.module_graph.read().unwrap();
        let mut module_ids: Vec<String> = module_graph
            .graph
            .node_weights()
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
        let mut references: Vec<(String, String)> = module_graph
            .graph
            .edge_references()
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
        references.sort_by_key(|(source, target)| format!("{} -> {}", source, target));

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
