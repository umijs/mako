use std::collections::BTreeMap;
use std::iter::once;
use std::sync::Arc;

use anyhow::anyhow;

use crate::compiler::Context;
use crate::module::generate_module_id;
use crate::plugin::Plugin;

pub struct CentralChunkEnsure {}

pub fn module_ensure_map(context: &Arc<Context>) -> anyhow::Result<BTreeMap<String, Vec<String>>> {
    let mg = context
        .module_graph
        .read()
        .map_err(|e| anyhow!("Read_Module_Graph_error:\n{:?}", e))?;
    let cg = context
        .chunk_graph
        .read()
        .map_err(|e| anyhow!("Read_Chunk_Graph_error:\n{:?}", e))?;

    let mut chunk_async_map: BTreeMap<String, Vec<String>> = Default::default();

    mg.modules().iter().for_each(|module| {
        let be_dynamic_imported = mg
            .get_dependents(&module.id)
            .iter()
            .any(|(_, dep)| dep.resolve_type.is_dynamic_esm());

        if be_dynamic_imported {
            cg.get_async_chunk_for_module(&module.id)
                .iter()
                .for_each(|chunk| {
                    let deps_chunks = cg
                        .sync_dependencies_chunk(&chunk.id)
                        .iter()
                        .chain(once(&chunk.id))
                        .map(|chunk_id| chunk_id.generate(context))
                        .collect::<Vec<_>>();

                    chunk_async_map.insert(generate_module_id(&module.id.id, context), deps_chunks);
                });
        }
    });

    Ok(chunk_async_map)
}

impl Plugin for CentralChunkEnsure {
    fn name(&self) -> &str {
        "dev_ensure2"
    }
    fn runtime_plugins(&self, context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        let chunk_async_map = module_ensure_map(context)?;

        // TODO: compress the map to reduce duplicated chunk ids
        let ensure_map = serde_json::to_string(&chunk_async_map)?;

        let runtime = format!(
            r#"
(function(){{
  let map = {ensure_map};
  requireModule.updateEnsure2Map = function(newMapping) {{
    map = newMapping;
  }};
  requireModule.ensure2 = function(chunkId){{
    let toEnsure = map[chunkId];
    if (toEnsure) {{
      return Promise.all(toEnsure.map(function(c){{ return requireModule.ensure(c); }}))
    }}else{{
      return Promise.resolve();
    }}
  }};
}})();
"#
        );

        Ok(vec![runtime])
    }

    fn hmr_runtime_updates(&self, _context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        let map = module_ensure_map(_context)?;

        let update_mapping = format!(
            "runtime.updateEnsure2Map({});",
            serde_json::to_string(&map)?
        );

        Ok(vec![update_mapping])
    }
}
