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
use crate::resolve::{get_resolvers, resolve, ResolverResource, Resolvers};
use crate::transform::transform;
use crate::util::create_thread_pool;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct GenericError(pub String);

#[derive(Debug)]
pub struct Task {
    pub path: String,
    pub parent_resource: Option<ResolverResource>,
    pub is_entry: bool,
}

pub type ModuleDeps = Vec<(ResolverResource, Dependency)>;

impl Compiler {
    pub fn build(&self) -> Result<()> {
        debug!("build");
        let t_build = Instant::now();
        let module_ids = self.build_module_graph()?;
        let t_build = t_build.elapsed();
        println!(
            "{} modules transformed in {}ms.",
            module_ids.len(),
            t_build.as_millis()
        );
        debug!("build done in {}ms", t_build.as_millis());
        Ok(())
    }

    fn build_module_graph(&self) -> Result<HashSet<ModuleId>> {
        debug!("build module graph");

        let entries: Vec<&PathBuf> = self.context.config.entry.values().collect();

        let resolvers = Arc::new(get_resolvers(&self.context.config));
        let (pool, rs, rr) = create_thread_pool::<Result<ModuleId>>();

        for entry in entries {
            let mut entry = entry.to_str().unwrap().to_string();
            if self.context.config.hmr
                && self.context.config.mode == Mode::Development
                && self.context.args.watch
            {
                entry = format!("{}?hmr", entry);
            }

            Self::build_module_graph_threaded(
                pool.clone(),
                self.context.clone(),
                Task {
                    path: entry,
                    parent_resource: None,
                    is_entry: true,
                },
                rs.clone(),
                resolvers.clone(),
            );
        }

        drop(rs);

        let mut errors = vec![];
        let mut module_ids = HashSet::new();
        for r in rr {
            match r {
                Ok(module_id) => {
                    module_ids.insert(module_id);
                }
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
                    errors.push(err);
                }
            }
        }

        if !errors.is_empty() {
            eprintln!("{}", "Build failed.".to_string().red());
            return Err(anyhow!(GenericError(errors.join(", "))));
        }

        Ok(module_ids)
    }

    pub fn build_module_graph_threaded(
        pool: Arc<ThreadPool>,
        context: Arc<Context>,
        task: Task,
        rs: Sender<Result<ModuleId>>,
        resolvers: Arc<Resolvers>,
    ) {
        let pool_clone = pool.clone();

        pool.spawn(move || {
            let (module, deps) = match Compiler::build_module(&context, &task, resolvers.clone()) {
                Ok(r) => r,
                Err(e) => {
                    rs.send(Err(e)).unwrap();
                    return;
                }
            };

            // record modules with missing deps
            if context.args.watch {
                if module.info.clone().unwrap().missing_deps.is_empty() {
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
                rs.send(Ok(module_id.clone())).unwrap();
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
                                Self::build_module_graph_threaded(
                                    pool_clone.clone(),
                                    context.clone(),
                                    Task {
                                        path: resolved_path,
                                        parent_resource: Some(resource),
                                        // parent_module_id: None,
                                        is_entry: false,
                                    },
                                    rs.clone(),
                                    resolvers.clone(),
                                );
                            }
                            // 拿到依赖之后需要直接添加 module 到 module_graph 里，不能等依赖 build 完再添加
                            // 由于是异步处理各个模块，后者会导致大量重复任务的 build_module 任务（3 倍左右）
                            rs.send(Ok(module.id.clone())).unwrap();
                            module_graph.add_module(module);
                        }
                        Err(err) => {
                            panic!("create module failed: {:?}", err);
                        }
                    }
                }
                module_graph.add_dependency(&module_id, &dep_module_id, dependency);
            });
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
        task: &Task,
        resolvers: Arc<Resolvers>,
    ) -> Result<(Module, ModuleDeps)> {
        let module_id = ModuleId::new(task.path.clone());
        let request = parse_path(&task.path)?;

        // load
        let content = load(&request, task.is_entry, context)?;

        // parse
        let mut ast = parse(&content, &request, context)?;

        // check ast
        context
            .plugin_driver
            .check_ast(&PluginCheckAstParam { ast: &ast }, context)?;

        // transform
        transform(&mut ast, context, task, &resolvers)?;

        // 在此之前需要把所有依赖都和模块关联起来，并且需要使用 resolved source
        // analyze deps
        let deps = analyze_deps(&ast, context)?;
        for dep in &deps {
            // e.g. file-loader!./file.txt
            if dep.source.contains("-loader!")
            // e.g. file-loader?esModule=false!./src-noconflict/theme-kr_theme.js
                || (dep.source.contains("-loader?") && dep.source.contains('!'))
            {
                return Err(anyhow!(
                    "webpack loader syntax is not supported, since found dep {:?} in {:?}",
                    dep.source,
                    task.path,
                ));
            }
        }

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

                    let resolved = resolved_resource.get_resolved_path();
                    let _external = resolved_resource.get_external();
                    let _id = ModuleId::new(resolved.clone());
                    // let id_str = id.generate(&context);

                    // let used_source = resolved_deps_to_source
                    //     .entry(id_str.clone())
                    //     .or_insert_with(|| dep.source.clone());

                    dependencies_resource.push((resolved_resource, dep.clone()));
                    // if dep.source.eq(used_source) {
                    // } else {
                    //     // duplicated_source_to_source_map
                    //     //     .insert(dep.source.clone(), used_source.clone());
                    // }
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
        let module = Module::new(module_id, task.is_entry, Some(info));

        Ok((module, dependencies_resource))
    }
}

fn is_async_module(path: &str) -> bool {
    // wasm should be treated as an async module
    ["wasm"].contains(&ext_name(path).unwrap_or(""))
}

pub fn parse_path(path: &str) -> Result<FileRequest> {
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
    use super::parse_path;

    #[test]
    fn test_parse_path() {
        let result = parse_path("foo").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(result.query, vec![]);

        let result = parse_path("foo?bar=1&hoo=2").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(
            result.query.first().unwrap(),
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
            result.query.first().unwrap(),
            &("bar".to_string(), "".to_string())
        );
        assert!(result.has_query("bar"));
    }
}
