use std::cmp::Ordering;

use petgraph::Direction::Incoming;

use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;

pub fn compare_modules_by_incoming_edges(
    module_graph: &ModuleGraph,
    a: &ModuleId,
    b: &ModuleId,
) -> std::cmp::Ordering {
    module_graph
        .get_edges_count(b, Incoming)
        .cmp(&module_graph.get_edges_count(a, Incoming))
}

pub fn assign_numeric_ids<T>(
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
