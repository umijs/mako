use super::ModuleFederationPlugin;
use crate::generate::chunk::ChunkType;
use crate::generate::chunk_graph::ChunkGraph;
use crate::module_graph::ModuleGraph;

impl ModuleFederationPlugin {
    pub(super) fn connect_provide_shared_to_container(
        &self,
        chunk_graph: &mut ChunkGraph,
        _module_graph: &mut ModuleGraph,
    ) {
        let entry_chunks = chunk_graph
            .get_chunks()
            .into_iter()
            .filter_map(|c| {
                if matches!(c.chunk_type, ChunkType::Entry(_, _, false)) {
                    Some(c.id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let provide_shared_map = self.shared_dependency_map.read().unwrap();

        let provide_shared_in_chunks = provide_shared_map
            .iter()
            .map(|m| {
                chunk_graph
                    .get_chunk_for_module(&m.0.as_str().into())
                    .unwrap()
                    .id
                    .clone()
            })
            .collect::<Vec<_>>();

        entry_chunks.iter().for_each(|ec| {
            provide_shared_in_chunks.iter().for_each(|c| {
                chunk_graph.add_edge(ec, c);
            });
        });
    }
}
