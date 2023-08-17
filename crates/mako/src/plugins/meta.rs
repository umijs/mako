use std::fs::write;
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;

use crate::compiler::Context;
use crate::module::ResolveType;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;

pub struct MetaPlugin {}

impl Plugin for MetaPlugin {
    fn name(&self) -> &str {
        "meta"
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
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

        let meta = serde_json::to_string_pretty(&serde_json::json!(ModuleGraphOutput { modules }))
            .unwrap();

        write(context.config.output.path.join("meta.json"), meta).unwrap();

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
