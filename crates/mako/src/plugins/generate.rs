use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::plugin::Plugin;

pub struct BundleGenerator {}

impl Plugin for BundleGenerator {
    fn name(&self) -> &str {
        "bundle_generator"
    }

    fn generate(&self, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
    }
}
