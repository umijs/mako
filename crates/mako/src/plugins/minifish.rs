mod inject;
mod unsimplify;

use std::path::PathBuf;
use std::sync::Arc;

pub(crate) use inject::Inject;
use inject::MyInjector;
use mako_core::anyhow::{anyhow, Result};
use mako_core::collections::HashMap;
use mako_core::rayon::prelude::*;
use mako_core::swc_ecma_visit::VisitMutWith;
use serde::Serialize;
use unsimplify::UnSimplify;

use crate::compiler::Context;
use crate::load::Content::Assets;
use crate::load::{read_content, Asset, Content};
use crate::module::{Dependency as ModuleDependency, ModuleAst, ResolveType};
use crate::plugin::{Plugin, PluginLoadParam, PluginParseParam, PluginTransformJsParam};
use crate::plugins::bundless_compiler::to_dist_path;
use crate::stats::StatsJsonMap;

pub struct MinifishPlugin {
    pub mapping: HashMap<String, String>,
    pub meta_path: Option<PathBuf>,
    pub inject: Option<HashMap<String, Inject>>,
}

impl MinifishPlugin {}

impl Plugin for MinifishPlugin {
    fn name(&self) -> &str {
        "minifish_plugin"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if param.task.is_match(vec!["json", "json5"]) {
            let root = _context.root.clone();
            let to: PathBuf = param.task.request.path.clone().into();

            let relative = to
                .strip_prefix(root)
                .unwrap_or_else(|_| panic!("{:?} not under project root", to))
                .to_str()
                .unwrap();

            return match self.mapping.get(relative) {
                Some(js_content) => Ok(Some(Content::Js(js_content.to_string()))),

                None => {
                    let content = read_content(param.task.path.as_str())?;

                    let asset = Asset {
                        path: param.task.path.clone(),
                        content,
                    };

                    Ok(Some(Assets(asset)))
                }
            };
        }
        Ok(None)
    }

    fn parse(
        &self,
        param: &PluginParseParam,
        _context: &Arc<Context>,
    ) -> Result<Option<ModuleAst>> {
        if param.task.request.path.ends_with(".json") {
            if let Assets(_) = param.content {
                return Ok(Some(ModuleAst::None));
            }
        }

        Ok(None)
    }

    fn transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut mako_core::swc_ecma_ast::Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        if let Some(inject) = &self.inject {
            if inject.is_empty() {
                return Ok(());
            }

            let mut matched_injects = HashMap::default();

            for (k, i) in inject {
                if let Some(exclude) = &i.exclude {
                    if !exclude.is_match(param.path) {
                        matched_injects.insert(k.clone(), i);
                    }
                } else {
                    matched_injects.insert(k.clone(), i);
                }
            }

            if matched_injects.is_empty() {
                return Ok(());
            }

            ast.visit_mut_with(&mut MyInjector::new(param.unresolved_mark, matched_injects));
        }
        Ok(())
    }

    fn after_generate_transform_js(
        &self,
        _param: &PluginTransformJsParam,
        ast: &mut mako_core::swc_ecma_ast::Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        ast.visit_mut_with(&mut UnSimplify {});
        Ok(())
    }

    fn before_resolve(
        &self,
        deps: &mut Vec<ModuleDependency>,
        _context: &Arc<Context>,
    ) -> Result<()> {
        let src_root = _context
            .config
            .output
            .preserve_modules_root
            .to_str()
            .ok_or_else(|| {
                anyhow!(
                    "output.preserve_modules_root {:?} is not a valid utf8 string",
                    _context.config.output.preserve_modules_root
                )
            })?;

        if src_root.is_empty() {
            return Err(anyhow!(
                "output.preserve_modules_root cannot be empty in minifish plugin"
            ));
        }

        for dep in deps.iter_mut() {
            if dep.source.starts_with('/') {
                let mut resolve_as = dep.source.clone();
                resolve_as.replace_range(0..0, src_root);
                dep.resolve_as = Some(resolve_as);
            }
        }

        Ok(())
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        if let Some(meta_path) = &self.meta_path {
            let mg = context.module_graph.read().unwrap();

            let ids = mg.get_module_ids();

            let modules: Vec<_> = ids
                .par_iter()
                .map(|id| {
                    let deps: Vec<_> = mg
                        .get_dependencies(id)
                        .iter()
                        .map(|dep| Dependency {
                            module: dep.0.id.clone(),
                            import_type: dep.1.resolve_type,
                        })
                        .collect();

                    let filename = if id.id.ends_with(".json") {
                        to_dist_path(&id.id, context).to_string_lossy().to_string()
                    } else {
                        to_dist_path(&id.id, context)
                            .with_extension("js")
                            .to_string_lossy()
                            .to_string()
                    };

                    Module {
                        filename,
                        id: id.id.clone(),
                        dependencies: deps,
                    }
                })
                .collect();

            let meta =
                serde_json::to_string_pretty(&serde_json::json!(ModuleGraphOutput { modules }))
                    .unwrap();

            std::fs::create_dir_all(meta_path.parent().unwrap()).unwrap();

            std::fs::write(meta_path, meta)
                .map_err(|e| anyhow!("write meta file({}) error: {}", meta_path.display(), e))?;
        }

        Ok(None)
    }
}
#[derive(Serialize)]
struct ModuleGraphOutput {
    modules: Vec<Module>,
}

#[derive(Serialize)]
struct Module {
    filename: String,
    id: String,
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    module: String,
    import_type: ResolveType,
}
