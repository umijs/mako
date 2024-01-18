use std::collections::HashMap;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::pathdiff::diff_paths;
use mako_core::rayon::prelude::*;
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::GLOBALS;
use mako_core::swc_ecma_transforms::fixer::fixer;
use mako_core::swc_ecma_transforms::helpers::{Helpers, HELPERS};
use mako_core::swc_ecma_transforms::hygiene;
use mako_core::swc_ecma_transforms::hygiene::hygiene_with_config;
use mako_core::swc_ecma_transforms_modules::import_analysis::import_analyzer;
use mako_core::swc_ecma_transforms_modules::util::ImportInterop;
use mako_core::swc_ecma_visit::VisitMutWith;
use mako_core::swc_error_reporters::handler::try_with_handler;
use mako_core::tracing::warn;

use crate::ast::{js_ast_to_code, Ast};
use crate::compiler::{Args, Context};
use crate::config::Config;
use crate::module::{ModuleAst, ModuleId};
use crate::plugin::{Plugin, PluginTransformJsParam};
use crate::transformers::transform_dep_replacer::{DepReplacer, DependenciesToReplace};
use crate::transformers::transform_dynamic_import::DynamicImport;

pub struct BundlessCompiler {
    // pub fs_write: Option<>
}

impl BundlessCompiler {
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
        let to = context.config.output.path.join(&filename);
        let to = normalize_extension(to);

        context
            .plugin_driver
            .before_write_fs(&to, content.as_ref())
            .unwrap();

        if !context.config.output.skip_write {
            fs::write(to, content).unwrap();
        }
    }
}

impl Plugin for BundlessCompiler {
    fn name(&self) -> &str {
        "bundless_compiler"
    }

    fn modify_config(&self, config: &mut Config, root: &Path, _args: &Args) -> Result<()> {
        if config.output.preserve_modules {
            let preserve_path = config.output.preserve_modules_root.clone();

            if !preserve_path.is_absolute() {
                config.output.preserve_modules_root = root.join(preserve_path);
            }
        }

        Ok(())
    }

    fn generate(&self, context: &Arc<Context>) -> Result<Option<()>> {
        self.transform_all(context)?;

        let mg = context.module_graph.read().unwrap();

        let ids = mg.get_module_ids();

        // TODO try tokio fs later
        ids.iter().for_each(|id| {
            let target = to_dist_path(&id.id, context);
            create_dir_all(target.parent().unwrap()).unwrap();
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
                    let target = to_dist_path(&id.id, context);
                    self.write_to_dist(target, &info.raw, context);
                }
            }
        });

        Ok(Some(()))
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    mako_core::mako_profile_function!();

    module_ids
        .par_iter()
        .map(|module_id| {
            let module_graph = context.module_graph.read().unwrap();
            let deps = module_graph.get_dependencies(module_id);

            let module_dist_path = to_dist_path(&module_id.id, context)
                .parent()
                .unwrap()
                .to_path_buf();

            let resolved_deps = deps
                .clone()
                .into_iter()
                // .map(|(id, dep)| (dep.source.clone(), id.generate(context)))
                .map(|(id, dep)| {
                    let dep_dist_path = to_dist_path(&id.id, context);

                    let rel_path =
                        diff_paths(&dep_dist_path, &module_dist_path).ok_or_else(|| {
                            anyhow!(
                                "failed to get relative path from {:?} to {:?}",
                                dep_dist_path,
                                module_dist_path
                            )
                        })?;

                    let rel_path = normalize_extension(rel_path);

                    let replacement: String = {
                        let mut to_path = rel_path.to_str().unwrap().to_string();
                        if to_path.starts_with("./") || to_path.starts_with("../") {
                            to_path
                        } else {
                            to_path.insert_str(0, "./");
                            to_path
                        }
                    };

                    Ok((dep.source.clone(), (replacement.clone(), replacement)))
                })
                .collect::<Result<Vec<_>>>();

            let resolved_deps: HashMap<String, (String, String)> =
                resolved_deps?.into_iter().collect();

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

            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(())
}

pub fn transform_js_generate(
    module_id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &DependenciesToReplace,
    _is_entry: bool,
) {
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
                            //             context.meta.script.origin_comments.read().unwrap();
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
                                module_id,
                                to_replace: dep_map,
                                context,
                                unresolved_mark: ast.unresolved_mark,
                                top_level_mark: ast.top_level_mark,
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

                            context.plugin_driver.after_generate_transform_js(
                                &PluginTransformJsParam {
                                    handler,
                                    path: &module_id.id,
                                    top_level_mark,
                                    unresolved_mark: ast.unresolved_mark,
                                },
                                &mut ast.ast,
                                context,
                            )?;

                            Ok(())
                        })
                    })
                },
            )
        })
        .unwrap();
}

pub fn to_dist_path<P: AsRef<str>>(abs_path: P, context: &Arc<Context>) -> PathBuf {
    let str = abs_path.as_ref();

    if str.contains("node_modules") {
        let base_path = &context.root;
        let relative_path = diff_paths(str, base_path).unwrap();

        context.config.output.path.join(relative_path)
    } else {
        let preserve_path = &context.config.output.preserve_modules_root;
        let relative_path = diff_paths(str, preserve_path).unwrap();

        context.config.output.path.join(relative_path)
    }
}

fn normalize_extension(to: PathBuf) -> PathBuf {
    if let Some(ext) = to.extension() {
        let ext = ext.to_str().unwrap();

        return match ext {
            "js" | "json" => to,
            "mjs" => to.with_extension("mjs.js"),
            "cjs" => to.with_extension("cjs.js"),
            "jsx" | "tsx" | "ts" => to.with_extension("js"),
            _ => {
                warn!("unknown extension: {} will keep unchanged", to.display());

                return to;
            }
        };
    }
    to
}
