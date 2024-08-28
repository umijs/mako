use std::collections::BTreeMap;
use std::sync::Arc;

use crate::compiler::Context;
use crate::generate::chunk::ChunkType;
use crate::plugin::Plugin;

pub struct Ensure2 {}

impl Plugin for Ensure2 {
    fn name(&self) -> &str {
        "dev_ensure2"
    }
    fn runtime_plugins(&self, _context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        let cg = _context.chunk_graph.read().unwrap();

        let mut chunk_async_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

        cg.get_chunks()
            .into_iter()
            .filter(|chunk| chunk.chunk_type == ChunkType::Async)
            .for_each(|chunk| {
                let chunk_ids = {
                    [
                        cg.sync_dependencies_chunk(&chunk.id),
                        vec![chunk.id.clone()],
                    ]
                    .concat()
                    .iter()
                    .filter_map(|chunk_id| {
                        // skip empty chunk because it will not be generated
                        if cg.chunk(chunk_id).is_some_and(|c| !c.modules.is_empty()) {
                            Some(chunk_id.id.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                };
                chunk_async_map.insert(chunk.id.id.clone(), chunk_ids);
            });

        // TODO: compress the map to reduce duplicated chunk ids
        let ensure_map = serde_json::to_string(&chunk_async_map)?;

        let runtime = format!(
            r#"
(function(){{
  let map = {ensure_map};
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
}
