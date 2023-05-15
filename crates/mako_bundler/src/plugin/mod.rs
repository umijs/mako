use std::{any::Any, sync::Arc};

use crate::{config::Config, context::Context, generate::generate::GenerateParam};

pub mod plugin_driver;

/// define all plugin errors here
#[derive(Debug)]
pub enum BundleError {}

/// define plugin result
pub type Result<T, E = BundleError> = std::result::Result<T, E>;

/// define plugin trait
pub trait Plugin: Any + Send + Sync {
    /// define plugin name
    ///
    /// Note: it is recommended to prefix a namespace to avoid name conflicts, such as `mako:plugin-xxx`
    fn name(&self) -> &str;

    /// let plugin run before other plugin
    fn before(&self) -> &str {
        ""
    }

    /// config hook
    fn config(&self, _config: &mut Config) -> Result<Option<()>> {
        Ok(None)
    }

    fn generate_end(
        &self,
        _context: &Arc<Context>,
        _generate_param: &GenerateParam,
    ) -> Result<Option<()>> {
        Ok(None)
    }

    // write other plugin hooks here
    fn example_method(&self, _prefix: String) -> Result<Option<String>> {
        Ok(None)
    }
}
