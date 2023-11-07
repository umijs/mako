use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::rayon::prelude::*;
use serde::Serialize;

use crate::compiler::Context;
use crate::load::Content;
use crate::module::ResolveType;
use crate::plugin::{Plugin, PluginLoadParam};
use crate::stats::StatsJsonMap;

pub struct MinifishPlugin {
    pub mapping: HashMap<String, String>,
    pub meta_path: Option<PathBuf>,
    pub mock: bool,
}

impl MinifishPlugin {}

impl Plugin for MinifishPlugin {
    fn name(&self) -> &str {
        "minifish_plugin"
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

            return match self.mapping.get(relative) {
                Some(js_content) => Ok(Some(Content::Js(js_content.to_string()))),
                None => Ok(None),
            };
        }
        Ok(None)
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

                    Module {
                        filename: id.id.clone(),
                        dependencies: deps,
                    }
                })
                .collect();

            let meta =
                serde_json::to_string_pretty(&serde_json::json!(ModuleGraphOutput { modules }))
                    .unwrap();

            std::fs::write(meta_path, meta).unwrap();
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
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    module: String,
    import_type: ResolveType,
}
