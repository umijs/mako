use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::anyhow;
use mako_core::anyhow::Result;
use mako_core::colored::Colorize;
use mako_core::thiserror::Error;

use crate::analyze_deps_2::AnalyzeDeps;
use crate::ast_2::file::{Content, File};
use crate::compiler::{Compiler, Context};
use crate::load_2::Load;
use crate::module::{Module, ModuleId, ModuleInfo};
use crate::parse_2::Parse;
use crate::resolve::ResolverResource;
use crate::transform_2::Transform;
use crate::util::create_thread_pool;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("{:}\n{:}", "Build failed.".to_string().red().to_string(), errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))]
    BuildTasksError { errors: Vec<anyhow::Error> },
}

impl Compiler {
    pub fn build_2(&self, files: Vec<File>) -> Result<HashSet<ModuleId>> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }

        let (pool, rs, rr) = create_thread_pool::<Result<Module>>();
        let build_with_pool = |file: File, parent_resource: Option<ResolverResource>| {
            let rs = rs.clone();
            let context = self.context.clone();
            pool.spawn(move || {
                let result = Self::build_module_2(&file, parent_resource, context);
                let result = Self::handle_build_result(result, &file, parent_resource, context);
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
            }
            let module = build_result.unwrap();

            let mut module_graph = self.context.module_graph.write().unwrap();

            // update context.modules_with_missing_deps (watch only)
            // TODO

            // handle current module
            // TODO: 统一 entry 和非 entry 的处理逻辑
            // entry 也可以先 create_module，存 module_graph，build 完成后添加 info
            let info = module.info.as_ref().unwrap();
            if info.file.is_entry {
                module_ids.insert(module.id.clone());
                module_graph.add_module(module);
            } else {
                let m = module_graph.get_module_mut(&module.id).unwrap();
                // TODO: add_info > set_info
                m.add_info(module.info);
            }

            // handle deps
            for dep in info.deps.resolved_deps {
                let path = dep.resolver_resource.get_resolved_path();
                let is_external = dep.resolver_resource.get_external().is_some();
                let dep_module_id = ModuleId::new(path);
                if !module_graph.has_module(&dep_module_id) {
                    let module = if is_external {
                        Self::create_external_module(&dep.resolver_resource, self.context.clone())
                    } else {
                        Self::create_empty_module(&dep_module_id)
                    };
                    if !is_external {
                        count += 1;
                        let file = File::new(path.clone(), self.context.clone());
                        build_with_pool(file, Some(dep.resolver_resource.clone()));
                    }
                    // 拿到依赖之后需要直接添加 module 到 module_graph 里，不能等依赖 build 完再添加
                    // 是因为由于是异步处理各个模块，后者会导致大量重复任务的 build_module 任务（3 倍左右）
                    module_ids.insert(module.id.clone());
                    module_graph.add_module(module);
                }
                module_graph.add_dependency(&module.id, &dep_module_id, dep.dependency);
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

    fn create_external_module(
        resolved_resource: &ResolverResource,
        context: Arc<Context>,
    ) -> Module {
        let external_name = resolved_resource
            .get_external()
            // safe
            .unwrap();
        let external_script = resolved_resource.get_script();
        let path = format!("virtual:external_{}", external_name);
        let mut file = File::new(path, context);
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
        let ast = Parse::parse(&file, context.clone())
            // safe
            .unwrap();
        let info = ModuleInfo {
            file,
            ast,
            // TODO: update
            external: Some(external_name),
            is_async: external_script.is_some(),
            resolved_resource: Some(resolved_resource.clone()),
            // TODO: remove
            path,
            raw: file.get_content_raw(),
            ..Default::default()
        };
        let module_id = ModuleId::new(file.path.to_string_lossy().to_string());
        Module::new(module_id, false, Some(info))
    }

    fn create_error_module(
        file: &File,
        err: String,
        resolved_resource: Option<ResolverResource>,
        context: Arc<Context>,
    ) -> Result<Module> {
        let mut file = file.clone();
        let code = format!("throw new Error(`Module build failed:\n{:}`)", err);
        file.set_content(Content::Js(code));
        let ast = Parse::parse(&file, context.clone())?;
        let path = file.path.to_string_lossy().to_string();
        let module_id = ModuleId::new(path.clone());
        let info = ModuleInfo {
            file,
            ast,
            path,
            resolved_resource,
            raw: file.get_content_raw(),
            ..Default::default()
        };
        Ok(Module::new(module_id, false, Some(info)))
    }

    fn create_empty_module(module_id: &ModuleId) -> Module {
        Module::new(module_id.clone(), false, None)
    }

    pub fn handle_build_result(
        result: Result<Module>,
        file: &File,
        resolved_resource: Option<ResolverResource>,
        context: Arc<Context>,
    ) -> Result<Module> {
        if result.is_err() && context.args.watch {
            let module = Self::create_error_module(
                file,
                result.err().unwrap().to_string(),
                resolved_resource,
                context.clone(),
            )?;
            Ok(module)
        } else {
            result
        }
    }

    pub fn build_module_2(
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
        // TODO: update info
        let path = file.path.to_string_lossy().to_string();
        let module_id = ModuleId::new(path.clone());
        let info = ModuleInfo {
            file,
            deps,
            ast,
            // TODO: update
            external: None,
            raw_hash: 0,
            top_level_await: false,
            is_async: false,
            source_map_chain: vec![],
            resolved_resource: parent_resource, /* TODO: rename */
            // TODO: remove
            path,
            raw: file.get_content_raw(),
            missing_deps: HashMap::new(),
            ignored_deps: vec![],
            import_map: vec![],
            export_map: vec![],
            is_barrel: false,
            // TODO: use Default::default() after unnecessary fields are removed
            // ..Default::default()
        };
        let module = Module::new(module_id, false, Some(info));
        Ok(module)
    }
}
