pub(crate) mod analyze_deps;
pub(crate) mod load;
pub(crate) mod parse;
pub(crate) mod targets;
pub(crate) mod transform;

use std::collections::HashSet;
use std::sync::mpsc::channel;
use std::sync::Arc;

use anyhow::Result;
use colored::Colorize;
use rkyv::Deserialize;
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, Mark, SourceMap as CM, SyntaxContext, GLOBALS};
use swc_core::ecma::ast::Module as SwcModule;
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use thiserror::Error;
use tokio::time::Instant;

use crate::ast::file::{Content, File, JsContent};
use crate::ast::js_ast::JsAst;
use crate::compiler::{Compiler, Context};
use crate::generate::chunk_pot::util::hash_hashmap;
use crate::module::{Module, ModuleAst, ModuleId, ModuleInfo};
use crate::plugin::NextBuildParam;
use crate::resolve::ResolverResource;
use crate::utils::thread_pool;

pub struct CleanSyntaxContext;

impl VisitMut for CleanSyntaxContext {
    fn visit_mut_syntax_context(&mut self, ctxt: &mut SyntaxContext) {
        *ctxt = SyntaxContext::empty();
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error(
        "{:}\n{:}", "Build failed.".to_string().red().to_string(), errors.iter().map(| e | e.to_string()).collect::< Vec < _ >> ().join("\n")
    )]
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
                m.set_info(module.info);
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
                        ResolverResource::Virtual(_) | ResolverResource::Resolved(_) => {
                            let file = File::new(path.clone(), self.context.clone());

                            if self.context.plugin_driver.next_build(&NextBuildParam {
                                current_module: &module_id,
                                next_file: &file,
                                resource: &dep.resolver_resource,
                            }) {
                                count += 1;
                                build_with_pool(file, Some(dep.resolver_resource.clone()));
                            }

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
        file.set_content(Content::Js(JsContent {
            content: code,
            ..Default::default()
        }));
        let ast = parse::Parse::parse(&file, context)
            // safe
            .unwrap();
        let raw = file.get_content_raw();
        let info = ModuleInfo {
            file,
            ast,
            external: Some(external_name),
            is_async,
            resolved_resource: Some(resolved_resource.clone()),
            raw,
            ..Default::default()
        };
        let module_id = ModuleId::new(origin_path.to_string());
        Module::new(module_id, false, Some(info))
    }

    fn create_error_module(file: &File, err: String, context: Arc<Context>) -> Result<Module> {
        let mut file = file.clone();
        let code = format!("throw new Error(`Module build failed:\n{:}`)", err);
        file.set_content(Content::Js(JsContent {
            content: code,
            ..Default::default()
        }));
        let ast = parse::Parse::parse(&file, context.clone())?;
        let path = file.path.to_string_lossy().to_string();
        let module_id = ModuleId::new(path.clone());
        let raw = file.get_content_raw();
        let info = ModuleInfo {
            file,
            ast,
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
                Content::Js(JsContent {
                    content: "export {};".to_string(),
                    ..Default::default()
                }),
                context.clone(),
            );
            let ast = parse::Parse::parse(&file, context.clone()).unwrap();

            ModuleInfo {
                file,
                ast,
                is_ignored: true,
                ..Default::default()
            }
        };

        module.set_info(Some(info));

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
        let content = load::Load::load(&file, context.clone())?;
        file.set_content(content);

        let star = Instant::now();

        let mut hit = false;

        let (ast, deps) = if let Some(Content::Js(_)) = &file.content {
            let raw_hash = file.get_raw_hash();
            let file_path = file.relative_path.to_string_lossy().to_string();

            let ast = if let Some(bytes) = context.cache.get_ast(&file_path, raw_hash) {
                let loadtakes = star.elapsed().as_micros();
                hit = true;

                let archived = unsafe { rkyv::archived_root::<SwcModule>(&bytes) };

                let mut deserialized_module: SwcModule = archived
                    .deserialize(&mut rkyv::de::deserializers::SharedDeserializeMap::new())
                    .unwrap();

                deserialized_module.visit_mut_with(&mut CleanSyntaxContext {});

                let ast = GLOBALS.set(&context.meta.script.globals, || {
                    let top_level_mark = Mark::new();
                    let unresolved_mark = Mark::new();
                    let contains_top_level_await = false;

                    let cm = Lrc::new(CM::default());

                    cm.new_source_file(
                        FileName::Real(file.relative_path.to_path_buf()).into(),
                        file.get_content_raw(),
                    );

                    deserialized_module.visit_mut_with(&mut resolver(
                        unresolved_mark,
                        top_level_mark,
                        false,
                    ));

                    ModuleAst::Script(JsAst {
                        ast: deserialized_module,
                        cm,
                        unresolved_mark,
                        top_level_mark,
                        path: file.relative_path.to_string_lossy().to_string(),
                        contains_top_level_await,
                    })
                });

                ast
            } else {
                let mut ast = parse::Parse::parse(&file, context.clone())?;
                transform::Transform::transform(&mut ast, &file, context.clone())?;

                ast
            };

            // 3. transform

            // 4. analyze deps + resolve
            let deps = analyze_deps::AnalyzeDeps::analyze_deps(&ast, &file, context.clone())?;

            if !hit {
                let bs = rkyv::to_bytes::<SwcModule, 512>(ast.as_script_ast())
                    .unwrap()
                    .to_vec();

                context.cache.insert(&file_path, raw_hash, &bs).unwrap();
            }

            (ast, deps)
        } else {
            // no css section
            // 2. parse
            let mut ast = parse::Parse::parse(&file, context.clone())?;

            // 3. transform
            transform::Transform::transform(&mut ast, &file, context.clone())?;

            // 4. analyze deps + resolve
            let deps = analyze_deps::AnalyzeDeps::analyze_deps(&ast, &file, context.clone())?;

            (ast, deps)
        };

        // above is for cache optimization

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
            resolved_resource: parent_resource,
            source_map_chain,
            top_level_await,
            is_async,
            raw_hash,
            raw,
            ..Default::default()
        };
        let module = Module::new(module_id, is_entry, Some(info));
        Ok(module)
    }
}
