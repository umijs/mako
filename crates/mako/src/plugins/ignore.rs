use std::sync::Arc;

use anyhow::Result;
use regex::Regex;

use crate::compiler::Context;
use crate::module::Dependency;
use crate::plugin::Plugin;

pub struct IgnorePlugin {
    pub ignores: Vec<Regex>,
}

impl Plugin for IgnorePlugin {
    fn name(&self) -> &str {
        "simple_ignore"
    }

    fn before_resolve(&self, deps: &mut Vec<Dependency>, _context: &Arc<Context>) -> Result<()> {
        deps.retain(|dep| !self.ignores.iter().any(|ig| ig.is_match(&dep.source)));

        Ok(())
    }
}
