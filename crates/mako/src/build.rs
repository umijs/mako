use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use mako_core::anyhow::{anyhow, Result};
use mako_core::colored::Colorize;
use mako_core::rayon::ThreadPool;
use mako_core::swc_ecma_utils::contains_top_level_await;
use mako_core::thiserror::Error;
use mako_core::tracing::debug;
use mako_core::{anyhow, thiserror};

use crate::analyze_deps::analyze_deps;
use crate::ast::{build_js_ast, generate_code_frame};
use crate::chunk_pot::util::{hash_hashmap, hash_vec};
use crate::compiler::{Compiler, Context};
use crate::config::Mode;
use crate::load::{ext_name, load};
use crate::module::{Dependency, Module, ModuleAst, ModuleId, ModuleInfo};
use crate::parse::parse;
use crate::plugin::PluginCheckAstParam;
use crate::resolve::{resolve, ResolverResource};
use crate::task::{self, Task};
use crate::transform::transform;
use crate::util::create_thread_pool;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct GenericError(pub String);

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("{:}\n{:}", "Build failed.".to_string().red().to_string(), errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))]
    BuildTasksError { errors: Vec<anyhow::Error> },
}

pub type ModuleDeps = Vec<(ResolverResource, Dependency)>;

impl Compiler {
    pub fn build(&self) -> Result<()> {
        debug!("build");
        let t_build = Instant::now();
        let entries: Vec<&PathBuf> = self.context.config.entry.values().collect();
        let tasks = entries
            .iter()
            .map(|entry| {
                let mut entry = entry.to_str().unwrap().to_string();
                if self.context.config.hmr
                    && self.context.config.mode == Mode::Development
                    && self.context.args.watch
                {
                    entry = format!("{}?hmr", entry);
                }
                task::Task::new(task::TaskType::Entry(entry), None)
            })
            .collect::<Vec<_>>();
        let module_ids = self.build_tasks(tasks)?;
        let t_build = t_build.elapsed();
        println!(
            "{} modules transformed in {}ms.",
            module_ids.len(),
            t_build.as_millis()
        );
        Ok(())
    }

    pub fn build_tasks(&self, tasks: Vec<Task>) -> Result<HashSet<ModuleId>> {
        debug!("build tasks: {:?}", tasks);
        if tasks.is_empty() {
            return Ok(HashSet::new());
        }

        let (pool, rs, rr) = create_thread_pool::<Result<(Module, ModuleDeps, Task)>>();
        let mut count = 0;
        for task in tasks {
            count += 1;
            Self::build_with_pool(pool.clone(), self.context.clone(), task, rs.clone());
        }

        let mut errors = vec![];
        let mut module_ids = HashSet::new();
        for r in rr {
            count -= 1;
            match r {
                Ok((module, deps, task)) => {
                    let context = self.context.clone();
                    // record modules with missing deps
                    if context.args.watch {
                        if module.info.as_ref().unwrap().missing_deps.is_empty() {
                            context
                                .modules_with_missing_deps
                                .write()
                                .unwrap()
                                .retain(|id| id != &module.id.id);
                        } else {
                            context
                                .modules_with_missing_deps
                                .write()
                                .unwrap()
                                .push(module.id.id.clone());
                        }
                    }

                    // current module
                    let module_id = module.id.clone();
                    // 只有处理 entry 时，module 会不存在于 module_graph 里
                    // 否则，module 肯定已存在于 module_graph 里，只需要补充 info 信息即可
                    let mut module_graph = context.module_graph.write().unwrap();
                    if task.is_entry {
                        module_ids.insert(module_id.clone());
                        module_graph.add_module(module);
                    } else {
                        let m = module_graph.get_module_mut(&module_id).unwrap();
                        m.add_info(module.info);
                    }

                    // deps
                    deps.into_iter().for_each(|(resource, dep)| {
                        let resolved_path = resource.get_resolved_path();
                        let external = resource.get_external();
                        let is_external = external.is_some();
                        let dep_module_id = ModuleId::new(resolved_path.clone());
                        let dependency = dep;

                        if !module_graph.has_module(&dep_module_id) {
                            let module = Self::create_module(&resource, &dep_module_id, &context);
                            match module {
                                Ok(module) => {
                                    if !is_external {
                                        count += 1;
                                        Self::build_with_pool(
                                            pool.clone(),
                                            context.clone(),
                                            task::Task::new(
                                                task::TaskType::Normal(resolved_path),
                                                Some(resource),
                                            ),
                                            rs.clone(),
                                        );
                                    }
                                    // 拿到依赖之后需要直接添加 module 到 module_graph 里，不能等依赖 build 完再添加
                                    // 由于是异步处理各个模块，后者会导致大量重复任务的 build_module 任务（3 倍左右）
                                    module_ids.insert(module.id.clone());
                                    module_graph.add_module(module);
                                }
                                Err(err) => {
                                    panic!("Create module failed: {:?}", err);
                                }
                            }
                        }
                        module_graph.add_dependency(&module_id, &dep_module_id, dependency);
                    });
                }
                Err(err) => {
                    errors.push(err);
                }
            }

            if count == 0 {
                break;
            }
        }

        debug!("Build tasks done");
        drop(rs);

        if !errors.is_empty() {
            return Err(anyhow!(BuildError::BuildTasksError { errors }));
        }

        Ok(module_ids)
    }

    pub fn build_with_pool(
        pool: Arc<ThreadPool>,
        context: Arc<Context>,
        task: task::Task,
        rs: Sender<Result<(Module, ModuleDeps, Task)>>,
    ) {
        pool.spawn(move || {
            let result = Self::build_module(&context, task);
            rs.send(result).unwrap();
        });
    }

    pub fn create_module(
        resource: &ResolverResource,
        dep_module_id: &ModuleId,
        context: &Arc<Context>,
    ) -> Result<Module> {
        let external = resource.get_external();
        let resolved_path = resource.get_resolved_path();
        let script = resource.get_script();
        let module = match external {
            Some(external) => {
                let has_script = script.is_some();

                let code = if let Some(url) = script {
                    format!(
                        r#"
module.exports = new Promise((resolve, reject) => {{
    __mako_require__.loadScript('{}', (e) => e.type === 'load' ? resolve() : reject(e));
}}).then(() => {});
"#,
                        url, external
                    )
                } else {
                    format!("module.exports = {};", external)
                };

                let ast = build_js_ast(
                    format!("external_{}", &resolved_path).as_str(),
                    code.as_str(),
                    context,
                )?;

                Module::new(
                    dep_module_id.clone(),
                    false,
                    Some(ModuleInfo {
                        ast: ModuleAst::Script(ast),
                        path: resolved_path,
                        external: Some(external),
                        raw: code,
                        raw_hash: 0,
                        resolved_resource: Some(resource.clone()),
                        missing_deps: HashMap::new(),
                        ignored_deps: vec![],
                        top_level_await: false,
                        is_async: has_script,
                    }),
                )
            }
            None => Module::new(dep_module_id.clone(), false, None),
        };
        Ok(module)
    }

    pub fn build_module(
        context: &Arc<Context>,
        task: task::Task,
    ) -> Result<(Module, ModuleDeps, Task)> {
        // load
        let content = load(&task, context)?;

        // parse
        let mut ast = parse(&content, &task, context)?;

        // check ast
        context
            .plugin_driver
            .check_ast(&PluginCheckAstParam { ast: &ast }, context)?;

        // transform
        transform(&mut ast, context, &task)?;

        // 在此之前需要把所有依赖都和模块关联起来，并且需要使用 resolved source
        // analyze deps
        let deps = analyze_deps(&ast, &task, context)?;

        // resolve
        let mut dep_resolve_err = None;
        let mut dependencies_resource = Vec::new();
        let mut missing_deps = HashMap::new();
        let mut ignored_deps = Vec::new();

        for dep in deps {
            let ret = resolve(&task.path, &dep, &context.resolvers, context);
            match ret {
                Ok(resolved_resource) => {
                    if matches!(resolved_resource, ResolverResource::Ignored) {
                        ignored_deps.push(dep.source.clone());
                        continue;
                    }
                    dependencies_resource.push((resolved_resource, dep.clone()));
                }
                Err(_) => {
                    // 获取 本次引用 和 上一级引用 路径
                    missing_deps.insert(dep.source.clone(), dep.clone());
                    dep_resolve_err =
                        Some((task.path.clone(), dep.source, dep.resolve_type, dep.span));
                }
            }
        }

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
            if context.args.watch {
                eprintln!("{}", err);
            } else {
                return Err(anyhow::anyhow!(err));
            }
        }

        // whether to contains top-level-await
        let top_level_await = {
            if let ModuleAst::Script(ast) = &ast {
                contains_top_level_await(&ast.ast)
            } else {
                false
            }
        };

        let raw_hash = content
            .raw_hash(context.config_hash)
            .wrapping_add(hash_hashmap(&missing_deps).wrapping_add(hash_vec(&ignored_deps)));

        // create module info
        let info = ModuleInfo {
            ast,
            path: task.path.clone(),
            external: None,
            raw: content.raw(),
            raw_hash,
            missing_deps,
            ignored_deps,
            top_level_await,
            is_async: top_level_await || is_async_module(&task.path),
            resolved_resource: task.parent_resource.clone(),
        };
        let module_id = ModuleId::new(task.path.clone());
        let module = Module::new(module_id, task.is_entry, Some(info));

        Ok((module, dependencies_resource, task))
    }
}

fn is_async_module(path: &str) -> bool {
    // wasm should be treated as an async module
    ["wasm"].contains(&ext_name(path).unwrap_or(""))
}
