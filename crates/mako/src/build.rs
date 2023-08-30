use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;
use swc_ecma_utils::contains_top_level_await;
use tokio::sync::mpsc::error::TryRecvError;
use tracing::debug;

use crate::analyze_deps::analyze_deps;
use crate::ast::{build_js_ast, generate_code_frame};
use crate::compiler::{Compiler, Context};
use crate::config::Config;
use crate::load::{ext_name, load};
use crate::module::{Dependency, Module, ModuleAst, ModuleId, ModuleInfo};
use crate::parse::parse;
use crate::plugin::PluginDepAnalyzeParam;
use crate::resolve::{get_resolvers, resolve, ResolverResource, Resolvers};
use crate::transform::transform;
use crate::transform_after_resolve::transform_after_resolve;
use crate::transform_dep_replacer::DependenciesToReplace;

#[derive(Debug)]
pub struct Task {
    pub path: String,
    pub parent_resource: Option<ResolverResource>,
    pub is_entry: bool,
}

pub type ModuleDeps = Vec<(ResolverResource, Dependency)>;

impl Compiler {
    pub fn build(&self) {
        debug!("build");
        let t_build = Instant::now();
        let module_ids = self.build_module_graph();
        let t_build = t_build.elapsed();
        // build chunk map 应该放 generate 阶段
        // 和 chunk 相关的都属于 generate
        println!(
            "{} modules transformed in {}ms.",
            module_ids.len(),
            t_build.as_millis()
        );
        debug!("build done in {}ms", t_build.as_millis());
    }

    fn build_module_graph(&self) -> HashSet<ModuleId> {
        debug!("build module graph");

        let entries =
            get_entries(&self.context.root, &self.context.config).expect("entry not found");
        if entries.is_empty() {
            panic!("entry not found");
        }

        let resolvers = Arc::new(get_resolvers(&self.context.config));
        let mut queue: VecDeque<Task> = VecDeque::new();
        for entry in entries {
            queue.push_back(Task {
                path: entry.to_str().unwrap().to_string(),
                parent_resource: None,
                is_entry: true,
            });
        }

        self.build_module_graph_by_task_queue(&mut queue, resolvers)
    }

    pub fn build_module_graph_by_task_queue(
        &self,
        queue: &mut VecDeque<Task>,
        resolvers: Arc<Resolvers>,
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
                let resolvers = resolvers.clone();
                let context = self.context.clone();
                tokio::spawn({
                    active_task_count += 1;
                    module_count += 1;
                    let rs = rs.clone();
                    async move {
                        let ret = Compiler::build_module(context, task, resolvers);
                        rs.send(ret).expect("send task failed");
                    }
                });
            }
            match rr.try_recv() {
                Ok(ret) => {
                    let (module, deps, task) = match ret {
                        Ok(ret) => ret,
                        Err(err) => {
                            // unescape
                            let mut err = err
                                .to_string()
                                .replace("\\n", "\n")
                                .replace("\\u{1b}", "\u{1b}")
                                .replace("\\\\", "\\");
                            // remove first char and last char
                            if err.starts_with('"') && err.ends_with('"') {
                                err = err[1..err.len() - 1].to_string();
                            }
                            eprintln!("{}", "Build failed.".to_string().red());
                            eprintln!("{}", err);
                            panic!("build module failed");
                        }
                    };
                    let t = Instant::now();

                    // record modules with missing deps
                    if !module.info.clone().unwrap().missing_deps.is_empty() {
                        self.context
                            .modules_with_missing_deps
                            .write()
                            .unwrap()
                            .push(module.id.id.clone());
                    }

                    // current module
                    let module_id = module.id.clone();
                    // 只有处理 entry 时，module 会不存在于 module_graph 里
                    // 否则，module 肯定已存在于 module_graph 里，只需要补充 info 信息即可
                    if task.is_entry {
                        added_module_ids.insert(module_id.clone());
                        module_graph.add_module(module);
                    } else {
                        let m = module_graph.get_module_mut(&module_id).unwrap();
                        m.add_info(module.info);
                    }

                    // deps
                    deps.iter().for_each(|(resource, dep)| {
                        let resolved_path = resource.get_resolved_path();
                        let external = resource.get_external();
                        let is_external = external.is_some();
                        let dep_module_id = ModuleId::new(resolved_path.clone());
                        let dependency = dep.clone();

                        if !module_graph.has_module(&dep_module_id) {
                            let module = self.create_module(resource, &dep_module_id);
                            match module {
                                Ok(module) => {
                                    if !is_external {
                                        queue.push_back(Task {
                                            path: resolved_path,
                                            parent_resource: Some(resource.clone()),
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
                        debug!("build time in main thread: {}ms", t_main_thread / 1000);
                        debug!("module count: {}", module_count);
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
        resource: &ResolverResource,
        dep_module_id: &ModuleId,
    ) -> Result<Module> {
        let external = resource.get_external();
        let resolved_path = resource.get_resolved_path();
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
                        raw_hash: 0,
                        resolved_resource: Some(resource.clone()),
                        missing_deps: HashMap::new(),
                        top_level_await: false,
                        is_async: false,
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
        resolvers: Arc<Resolvers>,
    ) -> Result<(Module, ModuleDeps, Task)> {
        let module_id = ModuleId::new(task.path.clone());
        let request = parse_path(&task.path)?;

        // load
        let content = load(&request, task.is_entry, &context)?;

        // parse
        let mut ast = parse(&content, &request, &context)?;

        // transform
        transform(&mut ast, &context, &task, &resolvers)?;

        // 在此之前需要把所有依赖都和模块关联起来，并且需要使用 resolved source
        // analyze deps
        let deps = analyze_deps(&ast)?;
        let mut deps_analyze_param = PluginDepAnalyzeParam { deps, ast: &ast };
        context
            .plugin_driver
            .analyze_deps(&mut deps_analyze_param)?;

        let deps = deps_analyze_param.deps;

        // resolve
        let mut dep_resolve_err = None;
        let mut dependencies_resource = Vec::new();
        let mut resolved_deps_to_source = HashMap::<String, String>::new();
        let mut duplicated_source_to_source_map = HashMap::new();

        let mut missing_dependencies = HashMap::new();

        for dep in deps {
            let ret = resolve(&task.path, &dep, &resolvers, &context);
            match ret {
                Ok(resolved_resource) => {
                    let resolved = resolved_resource.get_resolved_path();
                    let _external = resolved_resource.get_external();
                    let id = ModuleId::new(resolved.clone());
                    let id_str = id.generate(&context);

                    let used_source = resolved_deps_to_source
                        .entry(id_str.clone())
                        .or_insert_with(|| dep.source.clone());

                    if dep.source.eq(used_source) {
                        dependencies_resource.push((resolved_resource, dep.clone()));
                    } else {
                        duplicated_source_to_source_map
                            .insert(dep.source.clone(), used_source.clone());
                    }
                }
                Err(_) => {
                    // 获取 本次引用 和 上一级引用 路径
                    missing_dependencies.insert(dep.source.clone(), dep.clone());
                    dep_resolve_err =
                        Some((task.path.clone(), dep.source, dep.resolve_type, dep.span));
                }
            }
        }

        if context.config.mode == crate::config::Mode::Production {
            if let Some(e) = dep_resolve_err {
                // resolve 报错时的 target 和 source
                let target = e.0;
                let source = e.1;
                let span = e.3;
                // 使用 hasMap 记录循环依赖
                let mut target_map: HashMap<String, i32> = HashMap::new();
                target_map.insert(target, 1);

                let mut err = format!("Module not found: Can't resolve '{}'", source);

                if let Some(span) = span {
                    err = generate_code_frame(span, &err, context.meta.script.cm.clone());
                }

                // let id = ModuleId::new(target.clone());
                // let module_graph = context.module_graph.read().unwrap();

                // //  当 entry resolve 文件失败时，get_targets 自身会失败
                // if module_graph.get_module(&id).is_some() {
                //     let mut targets: Vec<ModuleId> = module_graph.dependant_module_ids(&id);
                //     // 循环找 target
                //     while !targets.is_empty() {
                //         let target_module_id = targets[0].clone();
                //         targets = module_graph.dependant_module_ids(&target_module_id);
                //         source = target.clone();
                //         target = target_module_id.id;
                //         // 拼接引用堆栈 string
                //         err = format!("{}  -> Resolve \"{}\" from \"{}\" \n", err, source, target);

                //         if target_map.contains_key(&target) {
                //             // 存在循环依赖
                //             err = format!("{}  -> \"{}\" 中存在循环依赖", err, target);
                //             break;
                //         } else {
                //             target_map.insert(target.clone(), 1);
                //         }
                //     }
                //     // 调整格式
                //     err = format!("{} \n", err);
                // }
                return Err(anyhow::anyhow!(err));
            }
        }

        // transform to replace deps
        // ref: https://github.com/umijs/mako/issues/311
        if !duplicated_source_to_source_map.is_empty() {
            let deps_to_replace = DependenciesToReplace {
                missing: HashMap::new(),
                resolved: duplicated_source_to_source_map,
            };
            transform_after_resolve(&mut ast, &context, &task, &deps_to_replace)?;
        }

        // whether to contains top-level-await
        let top_level_await = {
            if let ModuleAst::Script(ast) = &ast {
                contains_top_level_await(&ast.ast)
            } else {
                false
            }
        };

        // create module info
        let info = ModuleInfo {
            ast,
            path: task.path.clone(),
            external: None,
            raw_hash: content.raw_hash(),
            missing_deps: missing_dependencies,
            top_level_await,
            is_async: top_level_await || is_async_module(&task.path),
            resolved_resource: task.parent_resource.clone(),
        };
        let module = Module::new(module_id, task.is_entry, Some(info));

        Ok((module, dependencies_resource, task))
    }
}

fn is_async_module(path: &str) -> bool {
    // wasm should be treated as an async module
    ["wasm"].contains(&ext_name(path).unwrap())
}

pub fn get_entries(root: &Path, config: &Config) -> Option<Vec<std::path::PathBuf>> {
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

fn parse_path(path: &str) -> Result<FileRequest> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    Ok(FileRequest {
        path: path.to_string(),
        query: query_vec,
    })
}

#[derive(Debug)]
pub struct FileRequest {
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl FileRequest {
    pub fn has_query(&self, key: &str) -> bool {
        self.query.iter().any(|(k, _)| *k == key)
    }
}

#[cfg(test)]
mod tests {
    use petgraph::prelude::EdgeRef;
    use petgraph::visit::IntoEdgeReferences;

    use super::parse_path;
    use crate::compiler;
    use crate::config::Config;

    #[test]
    fn test_parse_path() {
        let result = parse_path("foo").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(result.query, vec![]);

        let result = parse_path("foo?bar=1&hoo=2").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(
            result.query.get(0).unwrap(),
            &("bar".to_string(), "1".to_string())
        );
        assert_eq!(
            result.query.get(1).unwrap(),
            &("hoo".to_string(), "2".to_string())
        );
        assert!(result.has_query("bar"));
        assert!(result.has_query("hoo"));
        assert!(!result.has_query("foo"));

        let result = parse_path("foo?bar").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(
            result.query.get(0).unwrap(),
            &("bar".to_string(), "".to_string())
        );
        assert!(result.has_query("bar"));
    }

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

    // TODO: add this test case back
    // #[tokio::test(flavor = "multi_thread")]
    // #[should_panic]
    // async fn test_build_panic_resolve() {
    //     build("test/build/panic-resolve");
    // }

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
                .map(|(source, target)| { format!("{} -> {}", source, target) })
                .collect::<Vec<String>>()
                .join(","),
            "index.css -> foo.css,index.ts -> index.css".to_string()
        );
    }

    fn build(base: &str) -> (Vec<String>, Vec<(String, String)>) {
        let current_dir = std::env::current_dir().unwrap();
        let pnpm_dir = current_dir.join("node_modules/.pnpm");
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        let compiler = compiler::Compiler::new(config, root.clone(), Default::default());
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
