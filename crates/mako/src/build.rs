use std::collections::HashSet;
use std::sync::mpsc::channel;
use std::sync::Arc;

use mako_core::anyhow;
use mako_core::anyhow::Result;
use mako_core::colored::Colorize;
use mako_core::thiserror::Error;

use crate::analyze_deps::AnalyzeDeps;
use crate::ast_2::file::{Content, File};
use crate::chunk_pot::util::hash_hashmap;
use crate::compiler::{Compiler, Context};
use crate::load::Load;
use crate::module::{Module, ModuleAst, ModuleId, ModuleInfo};
use crate::parse::Parse;
use crate::resolve::ResolverResource;
use crate::thread_pool;
use crate::transform::Transform;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("{:}\n{:}", "Build failed.".to_string().red().to_string(), errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))]
    BuildTasksError { errors: Vec<anyhow::Error> },
}

impl Compiler {
    pub fn build(&self, files: Vec<File>) -> Result<HashSet<ModuleId>> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }

        let (rs, rr) = channel::<Result<Module>>();

        let build_with_pool = |file: File, parent_resource: Option<ResolverResource>| {
            let rs = rs.clone();
            let context = self.context.clone();
            thread_pool::spawn(move || {
                let result = Self::build_module(&file, parent_resource, context.clone());
                let result = Self::handle_build_result(result, &file, context);
                rs.send(result).unwrap();
            });
        };
        let mut count = 0;
        for file in files {
            count += 1;
            build_with_pool(file, None);
        }

        let mut errors = vec![];
        let mut module_ids = HashSet::new();

        for build_result in rr {
            count -= 1;

            // handle build_module error
            if build_result.is_err() {
                errors.push(build_result.err().unwrap());
                if count == 0 {
                    break;
                } else {
                    continue;
                }
            }
            let module = build_result.unwrap();
            let module_id = module.id.clone();

            // update context.modules_with_missing_deps (watch only)
            if self.context.args.watch {
                if module.info.as_ref().unwrap().deps.missing_deps.is_empty() {
                    self.context
                        .modules_with_missing_deps
                        .write()
                        .unwrap()
                        .retain(|id| id != &module_id.id);
                } else {
                    self.context
                        .modules_with_missing_deps
                        .write()
                        .unwrap()
                        .push(module_id.id.clone());
                }
            }

            let mut module_graph = self.context.module_graph.write().unwrap();

            // handle current module
            let info = module.info.as_ref().unwrap();
            let resolved_deps = info.deps.resolved_deps.clone();
            let m = module_graph.get_module_mut(&module.id);
            if let Some(m) = m {
                // TODO: add_info > set_info
                m.add_info(module.info);
            } else {
                module_ids.insert(module.id.clone());
                module_graph.add_module(module);
            }

            // handle deps
            for dep in resolved_deps {
                let path = dep.resolver_resource.get_resolved_path();
                let dep_module_id = ModuleId::new(path.clone());
                if !module_graph.has_module(&dep_module_id) {
                    let module = match dep.resolver_resource {
                        ResolverResource::Resolved(_) => {
                            count += 1;

                            let file = File::new(path.clone(), self.context.clone());
                            build_with_pool(file, Some(dep.resolver_resource.clone()));

                            Self::create_empty_module(&dep_module_id)
                        }
                        ResolverResource::External(_) => Self::create_external_module(
                            &dep.resolver_resource,
                            self.context.clone(),
                        ),
                        ResolverResource::Ignored(_) => {
                            Self::create_ignored_module(&path, self.context.clone())
                        }
                    };

                    // 拿到依赖之后需要直接添加 module 到 module_graph 里，不能等依赖 build 完再添加
                    // 是因为由于是异步处理各个模块，后者会导致大量重复任务的 build_module 任务（3 倍左右）
                    module_ids.insert(module.id.clone());
                    module_graph.add_module(module);
                }
                module_graph.add_dependency(&module_id, &dep_module_id, dep.dependency);
            }
            if count == 0 {
                break;
            }
        }
        drop(rs);

        if !errors.is_empty() {
            return Err(anyhow::anyhow!(BuildError::BuildTasksError { errors }));
        }

        Ok(module_ids)
    }

    pub fn create_external_module(
        resolved_resource: &ResolverResource,
        context: Arc<Context>,
    ) -> Module {
        let external_name = resolved_resource
            .get_external()
            // safe
            .unwrap();
        let external_script = resolved_resource.get_script();
        let is_async = external_script.is_some();
        let origin_path = resolved_resource.get_resolved_path();
        let path = format!("virtual:external_{}", origin_path);
        let mut file = File::new(path.clone(), context.clone());
        let code = if let Some(url) = external_script {
            format!(
                r#"
module.exports = new Promise((resolve, reject) => {{
__mako_require__.loadScript('{}', (e) => e.type === 'load' ? resolve() : reject(e));
}}).then(() => {});
"#,
                url, external_name
            )
        } else {
            format!("module.exports = {};", external_name)
        };
        file.set_content(Content::Js(code));
        let ast = Parse::parse(&file, context)
            // safe
            .unwrap();
        let raw = file.get_content_raw();
        let info = ModuleInfo {
            file,
            ast,
            // TODO: update
            external: Some(external_name),
            is_async,
            resolved_resource: Some(resolved_resource.clone()),
            // TODO: remove
            path,
            raw,
            ..Default::default()
        };
        let module_id = ModuleId::new(origin_path.to_string());
        Module::new(module_id, false, Some(info))
    }

    fn create_error_module(file: &File, err: String, context: Arc<Context>) -> Result<Module> {
        let mut file = file.clone();
        let code = format!("throw new Error(`Module build failed:\n{:}`)", err);
        file.set_content(Content::Js(code));
        let ast = Parse::parse(&file, context.clone())?;
        let path = file.path.to_string_lossy().to_string();
        let module_id = ModuleId::new(path.clone());
        let raw = file.get_content_raw();
        let info = ModuleInfo {
            file,
            ast,
            path,
            raw,
            ..Default::default()
        };
        Ok(Module::new(module_id, false, Some(info)))
    }

    fn create_ignored_module(path: &str, context: Arc<Context>) -> Module {
        let module_id = ModuleId::new(path.to_owned());

        let mut module = Module::new(module_id, false, None);

        let info = {
            let file = File::with_content(
                path.to_owned(),
                Content::Js("export {};".to_string()),
                context.clone(),
            );
            let ast = Parse::parse(&file, context.clone()).unwrap();

            ModuleInfo {
                file,
                ast,
                ..Default::default()
            }
        };

        module.add_info(Some(info));

        module
    }

    pub fn create_empty_module(module_id: &ModuleId) -> Module {
        Module::new(module_id.clone(), false, None)
    }

    pub fn handle_build_result(
        result: Result<Module>,
        file: &File,
        context: Arc<Context>,
    ) -> Result<Module> {
        if result.is_err() && context.args.watch {
            let module = Self::create_error_module(
                file,
                result.err().unwrap().to_string(),
                context.clone(),
            )?;
            Ok(module)
        } else {
            result
        }
    }

    pub fn build_module(
        file: &File,
        parent_resource: Option<ResolverResource>,
        context: Arc<Context>,
    ) -> Result<Module> {
        // 1. load
        let mut file = file.clone();
        let content = Load::load(&file, context.clone())?;
        file.set_content(content);

        // 2. parse
        let mut ast = Parse::parse(&file, context.clone())?;

        // 3. transform
        Transform::transform(&mut ast, &file, context.clone())?;

        // 4. analyze deps + resolve
        let deps = AnalyzeDeps::analyze_deps(&ast, &file, context.clone())?;

        // 5. create module
        let path = file.path.to_string_lossy().to_string();
        let module_id = ModuleId::new(path.clone());
        let raw = file.get_content_raw();
        let is_entry = file.is_entry;
        let source_map_chain = file.get_source_map_chain(context.clone());
        let top_level_await = match &ast {
            ModuleAst::Script(ast) => ast.contains_top_level_await,
            _ => false,
        };
        let is_async_module = file.extname == "wasm";
        let is_async = is_async_module || top_level_await;

        // raw_hash is only used in watch mode
        // so we don't need to calculate when watch is off
        let raw_hash = if context.args.watch {
            file.get_raw_hash()
                .wrapping_add(hash_hashmap(&deps.missing_deps))
        } else {
            0
        };
        let info = ModuleInfo {
            file,
            deps,
            ast,
            resolved_resource: parent_resource, /* TODO: rename */
            source_map_chain,
            top_level_await,
            is_async,
            raw_hash,
            // TODO: remove
            path,
            raw,
            ..Default::default()
        };
        let module = Module::new(module_id, is_entry, Some(info));
        Ok(module)
    }
}
