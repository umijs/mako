use std::collections::HashMap;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use mako_core::anyhow::Result;
use mako_core::cached::proc_macro::cached;
use mako_core::pathdiff::diff_paths;
use mako_core::rayon::prelude::*;
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::GLOBALS;
use mako_core::swc_ecma_transforms::helpers::{Helpers, HELPERS};
use mako_core::swc_ecma_transforms::hygiene::hygiene_with_config;
use mako_core::swc_ecma_transforms::modules::import_analysis::import_analyzer;
use mako_core::swc_ecma_transforms::modules::util::ImportInterop;
use mako_core::swc_ecma_transforms::{fixer, hygiene};
use mako_core::swc_ecma_visit::VisitMutWith;
use mako_core::swc_error_reporters::handler::try_with_handler;

use crate::ast::{js_ast_to_code, Ast};
use crate::compiler::Context;
use crate::config::{Config, Mode};
use crate::load::{read_content, Content};
use crate::module::{ModuleAst, ModuleId};
use crate::plugin::{Plugin, PluginLoadParam};
use crate::transformers::transform_dep_replacer::{DepReplacer, DependenciesToReplace};
use crate::transformers::transform_dynamic_import::DynamicImport;

pub struct MinifishCompiler {
    minifish_map: HashMap<String, String>,
}

impl MinifishCompiler {
    pub fn new(_: &Config, root: &Path) -> Self {
        let map_file = root.join("_apcJsonContentMap.json");

        let content = read_content(map_file).unwrap();

        let minifish_map = serde_json::from_str::<HashMap<String, String>>(&content).unwrap();

        Self { minifish_map }
    }
    pub fn transform_all(&self, context: &Arc<Context>) -> Result<()> {
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        transform_modules(module_ids, context)?;
        Ok(())
    }

    pub fn write_to_dist<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(
        &self,
        filename: P,
        content: C,
        context: &Arc<Context>,
    ) {
        let to = context
            .config
            .output
            .path
            .join(filename)
            .with_extension("js");

        fs::write(to, content).unwrap();
    }
}

#[cached(result = true, key = "String", convert = r#"{ format!("{}", path) }"#)]
fn create_dir_all_with_cache(path: &str) -> io::Result<()> {
    create_dir_all(path)
}

impl Plugin for MinifishCompiler {
    fn name(&self) -> &str {
        "minifish_generator"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "json" | "json5") {
            let root = _context.root.clone();
            let to: PathBuf = param.path.clone().into();

            let relative = to
                .strip_prefix(root)
                .unwrap_or_else(|_| panic!("{:?} not under project root", to))
                .to_str()
                .unwrap();

            return match self.minifish_map.get(relative) {
                Some(js_content) => Ok(Some(Content::Js(js_content.to_string()))),
                None => Ok(None),
            };
        }
        Ok(None)
    }

    fn generate(&self, context: &Arc<Context>) -> Result<Option<()>> {
        self.transform_all(context)?;

        let mg = context.module_graph.read().unwrap();

        let ids = mg.get_module_ids();

        let _output = context.config.output.path.canonicalize().unwrap();

        // TODO try tokio fs later
        ids.iter().for_each(|id| {
            let target = to_dist_path(&id.id, context);
            create_dir_all_with_cache(target.parent().unwrap().to_str().unwrap()).unwrap();
        });

        ids.par_iter().for_each(|id| {
            let module = mg.get_module(id).expect("module not exits");

            let info = module.info.as_ref().expect("module info missing");

            match &info.ast {
                ModuleAst::Script(js_ast) => {
                    if module.id.id.ends_with(".json") {
                        // nothing
                        // todo: generate resolved AJSON
                    } else {
                        let (code, _) = js_ast_to_code(&js_ast.ast, context, "a.js")
                            .unwrap_or(("".to_string(), "".to_string()));

                        let target = to_dist_path(&id.id, context);

                        self.write_to_dist(target, code, context);
                    }
                }
                ModuleAst::Css(_style) => {}
                ModuleAst::None => {
                    //  nothing
                }
            }
        });

        Ok(Some(()))
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let module_dist_path = to_dist_path(&module_id.id, context)
            .parent()
            .unwrap()
            .to_path_buf();

        let resolved_deps: HashMap<String, String> = deps
            .clone()
            .into_iter()
            // .map(|(id, dep)| (dep.source.clone(), id.generate(context)))
            .map(|(id, dep)| {
                let dep_dist_path = to_dist_path(&id.id, context);

                let rel_path = diff_paths(dep_dist_path, &module_dist_path)
                    .unwrap()
                    .with_extension("js");

                let replacement: String = {
                    let mut to_path = rel_path.to_str().unwrap().to_string();
                    if to_path.starts_with('.') {
                        to_path
                    } else {
                        to_path.insert_str(0, "./");
                        to_path
                    }
                };

                (dep.source.clone(), replacement)
            })
            .collect();
        let _assets_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
            .collect();
        drop(module_graph);

        // let deps: Vec<(&ModuleId, &crate::module::Dependency)> =
        //     module_graph.get_dependencies(module_id);
        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let ast = &mut info.ast;

        let deps_to_replace = DependenciesToReplace {
            resolved: resolved_deps,
            missing: info.missing_deps.clone(),
            ignored: vec![],
        };

        if let ModuleAst::Script(ast) = ast {
            transform_js_generate(&module.id, context, ast, &deps_to_replace, module.is_entry);
        }
    });
    Ok(())
}

pub fn transform_js_generate(
    _id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &DependenciesToReplace,
    _is_entry: bool,
) {
    let _is_dev = matches!(context.config.mode, Mode::Development);
    GLOBALS
        .set(&context.meta.script.globals, || {
            try_with_handler(
                context.meta.script.cm.clone(),
                Default::default(),
                |handler| {
                    HELPERS.set(&Helpers::new(true), || {
                        HANDLER.set(handler, || {
                            let _unresolved_mark = ast.unresolved_mark;
                            let top_level_mark = ast.top_level_mark;
                            // let (code, ..) = js_ast_to_code(&ast.ast, context, "foo").unwrap();
                            // print!("{}", code);

                            // {
                            //     if context.config.minify
                            //         && matches!(context.config.mode, Mode::Production)
                            //     {
                            //         let comments =
                            //             context.meta.script.output_comments.read().unwrap();
                            //         let mut unused_statement_sweep =
                            //             UnusedStatementSweep::new(id, &comments);
                            //         ast.ast.visit_mut_with(&mut unused_statement_sweep);
                            //     }
                            // }

                            let import_interop = ImportInterop::Swc;
                            // FIXME: 执行两轮 import_analyzer + inject_helpers，第一轮是为了 module_graph，第二轮是为了依赖替换
                            ast.ast
                                .visit_mut_with(&mut import_analyzer(import_interop, true));
                            // ast.ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                            let mut dep_replacer = DepReplacer {
                                to_replace: dep_map,
                                context,
                            };
                            ast.ast.visit_mut_with(&mut dep_replacer);

                            let mut dynamic_import = DynamicImport { context };
                            ast.ast.visit_mut_with(&mut dynamic_import);

                            ast.ast
                                .visit_mut_with(&mut hygiene_with_config(hygiene::Config {
                                    top_level_mark,
                                    ..Default::default()
                                }));
                            ast.ast.visit_mut_with(&mut fixer(Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            )));

                            Ok(())
                        })
                    })
                },
            )
        })
        .unwrap();
}

pub fn to_dist_path<P: AsRef<str>>(abs_path: P, context: &Arc<Context>) -> PathBuf {
    let src_root = context
        .root
        .clone()
        .join("src")
        .to_str()
        .unwrap()
        .to_string();
    let npm_root = context
        .root
        .clone()
        .join("node_modules")
        .to_str()
        .unwrap()
        .to_string();

    let str = abs_path.as_ref();

    if str.contains(&src_root) {
        let relative_path = diff_paths(str, &src_root).unwrap();
        context.config.output.path.join(relative_path)
    } else if str.contains(&npm_root) {
        let relative_path = diff_paths(str, &context.root).unwrap();

        context.config.output.path.join(relative_path)
    } else {
        abs_path.as_ref().to_string().into()
    }
}
