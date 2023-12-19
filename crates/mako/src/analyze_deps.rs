use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};

use crate::compiler::Context;
use crate::module::{Dependency, ModuleAst};
use crate::plugin::PluginDepAnalyzeParam;

pub fn analyze_deps(
    ast: &ModuleAst,
    file_path: &String,
    context: &Arc<Context>,
) -> Result<Vec<Dependency>> {
    mako_core::mako_profile_function!();

    let mut analyze_deps_param = PluginDepAnalyzeParam { ast };

    let mut deps = context
        .plugin_driver
        .analyze_deps(&mut analyze_deps_param, context)?;

    context.plugin_driver.before_resolve(&mut deps, context)?;

    // check loader syntax
    // e.g. file-loader!./file.txt
    // e.g. file-loader?esModule=false!./src-noconflict/theme-kr_theme.js
    for dep in &deps {
        if dep.source.contains("-loader!")
            || (dep.source.contains("-loader?") && dep.source.contains('!'))
        {
            return Err(anyhow!(
                "webpack loader syntax is not supported, since found dep {:?} in {:?}",
                dep.source,
                file_path
            ));
        }
    }

    Ok(deps)
}
