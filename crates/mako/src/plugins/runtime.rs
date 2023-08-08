use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::plugin::Plugin;

pub struct MakoRuntime {}

impl Plugin for MakoRuntime {
    fn name(&self) -> &str {
        "mako/runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let plugins = vec![self.public_path(context)];
        Ok(plugins)
    }
}

impl MakoRuntime {
    fn public_path(&self, context: &Arc<Context>) -> String {
        let public_path = context.config.public_path.clone();
        let public_path = if public_path == "runtime" {
            "globalThis.publicPath".to_string()
        } else {
            format!("\"{}\"", public_path)
        };

        format!(
            r#"
  /* mako/runtime/publicPath */
  !function () {{
    requireModule.publicPath= {};
  }}();"#,
            public_path
        )
    }
}
