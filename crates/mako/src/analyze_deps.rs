use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::module::{Dependency, ModuleAst};
use crate::plugin::PluginDepAnalyzeParam;

pub fn analyze_deps(ast: &ModuleAst, context: &Arc<Context>) -> Result<Vec<Dependency>> {
    mako_core::mako_profile_function!();

    let mut analyze_deps_param = PluginDepAnalyzeParam { ast };

    let deps = context
        .plugin_driver
        .analyze_deps(&mut analyze_deps_param)?;

    Ok(deps)
}
