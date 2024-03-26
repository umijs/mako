use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::module_graph::ModuleGraph;
use crate::plugin::Plugin;

mod module;
mod module_side_effects_flag;
mod remove_useless_stmts;
mod shake;
mod statement_graph;

pub struct FarmTreeShake {}

impl Plugin for FarmTreeShake {
    fn name(&self) -> &str {
        "farm/tree-shake"
    }

    fn optimize_module_graph(
        &self,
        module_graph: &mut ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<()> {
        shake::optimize_farm(module_graph, context)?;
        Ok(())
    }
}
