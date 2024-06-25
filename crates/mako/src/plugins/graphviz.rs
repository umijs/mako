use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use petgraph::dot::{Config, Dot};
use petgraph::visit::{GraphProp, IntoEdgeReferences, IntoNodeReferences, NodeIndexable};

use crate::compiler::Context;
use crate::plugin::{Plugin, PluginGenerateEndParams};

pub struct Graphviz {}

impl Graphviz {
    fn write_graph<G, P>(dot_filename: P, graph: G) -> Result<()>
    where
        P: AsRef<Path>,
        G: IntoEdgeReferences + IntoNodeReferences + NodeIndexable + GraphProp,
        G::EdgeWeight: Debug,
        G::NodeWeight: Debug,
    {
        let dot = Dot::with_config(graph, &[Config::EdgeNoLabel]);
        let mut file = File::create(dot_filename.as_ref())
            .unwrap_or_else(|_| panic!("{} cant create", dot_filename.as_ref().display()));

        write!(file, "{:?}", dot)?;
        Ok(())
    }
}

impl Plugin for Graphviz {
    fn name(&self) -> &str {
        "graphviz"
    }

    fn generate_beg(&self, context: &Arc<Context>) -> Result<()> {
        Graphviz::write_graph(
            context.root.join("_mako_module_graph_origin.dot"),
            &context.module_graph.read().unwrap().graph,
        )?;
        Ok(())
    }

    fn before_optimize_chunk(&self, context: &Arc<Context>) -> Result<()> {
        Graphviz::write_graph(
            context.root.join("_mako_chunk_graph_origin.dot"),
            &context.chunk_graph.read().unwrap().graph,
        )?;
        Ok(())
    }

    fn generate_end(
        &self,
        _params: &PluginGenerateEndParams,
        context: &Arc<Context>,
    ) -> Result<Option<()>> {
        Graphviz::write_graph(
            context.root.join("_mako_chunk_graph_finale.dot"),
            &context.chunk_graph.read().unwrap().graph,
        )?;

        Graphviz::write_graph(
            context.root.join("_mako_module_graph_finale.dot"),
            &context.module_graph.read().unwrap().graph,
        )?;

        Ok(None)
    }
}
