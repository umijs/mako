use std::cmp::Ordering;

use petgraph::Direction::Incoming;

use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

fn get_edges_count(graph: &ModuleGraph, module_id: &ModuleId) -> usize {
    let node_index = graph.id_index_map.get(module_id).unwrap_or_else(|| {
        panic!(
            r#"from node "{}" does not exist in the module graph when remove edge"#,
            module_id.id
        )
    });
    graph.graph.edges_directed(*node_index, Incoming).count()
}

pub fn compare_modules_by_incomming_edges(
    module_graph: &ModuleGraph,
    a: &ModuleId,
    b: &ModuleId,
) -> std::cmp::Ordering {
    get_edges_count(module_graph, b).cmp(&get_edges_count(module_graph, a))
}

pub fn assign_numberous_ids<T>(
    mut items: Vec<T>,
    comparator: impl Fn(&T, &T) -> Ordering,
    mut assign_id: impl FnMut(&T, usize),
) {
    items.sort_unstable_by(comparator);

    items
        .iter()
        .enumerate()
        .for_each(|(i, item)| assign_id(item, i))
}
