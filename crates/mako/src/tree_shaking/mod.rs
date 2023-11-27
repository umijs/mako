pub mod analyze_statement;
pub mod defined_ident_collector;
pub mod module_side_effects_flag;
pub mod statement;
pub mod statement_graph;
#[allow(clippy::module_inception)]
pub mod tree_shaking;
pub mod tree_shaking_analyze;
pub mod tree_shaking_module;
pub mod unused_statement_cleanup;
pub mod unused_statement_marker;
pub mod unused_statement_sweep;
pub mod used_ident_collector;
