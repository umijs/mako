use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::tracing::debug;

use crate::compiler::Context;
use crate::module::ModuleId;
use crate::plugin::Plugin;

pub struct MakoRuntime {}

const DEFAULT_INTEROP: &str = include_str!(concat!(
    env!("MANIFEST_DIR"),
    "/../../node_modules/@swc/helpers/cjs/_interop_require_default.cjs"
));

const WILDCARD_INTEROP: &str = include_str!(concat!(
    env!("MANIFEST_DIR"),
    "/../../node_modules/@swc/helpers/cjs/_interop_require_wildcard.cjs"
));

const EXPORTS_ALL: &str = include_str!(concat!(
    env!("MANIFEST_DIR"),
    "/../../node_modules/@swc/helpers/cjs/_export_star.cjs"
));

impl Plugin for MakoRuntime {
    fn name(&self) -> &str {
        "mako/runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let plugins = vec![
            self.public_path(context),
            self.helper_runtime(context).unwrap(),
        ];
        Ok(plugins)
    }
}

impl MakoRuntime {
    fn public_path(&self, context: &Arc<Context>) -> String {
        let public_path = context.config.public_path.clone();
        let public_path = if public_path == "runtime" {
            "(typeof globalThis !== 'undefined' ? globalThis : self).publicPath || '/'".to_string()
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

    fn helper_runtime(&self, context: &Arc<Context>) -> Result<String> {
        let helpers = context.swc_helpers.lock().unwrap().get_helpers();
        debug!("swc helpers: {:?}", helpers);

        if helpers.is_empty() {
            return Ok("".to_string());
        }

        let helpers = helpers
            .into_iter()
            .map(|source| {
                let code = Self::get_swc_helper_code(&source).unwrap();
                let module_id: ModuleId = source.into();
                let module_id = module_id.generate(context);
                format!("\"{}\": {}", module_id, code)
            })
            .collect::<Vec<_>>()
            .join(",\n");

        Ok(format!(
            r#"
  /* mako/runtime/helpers */
  registerModules({{
    {}
  }});
        "#,
            helpers
        ))
    }

    fn get_swc_helper_code(path: &str) -> Result<String> {
        let code = match path {
            "@swc/helpers/_/_interop_require_default" => wrap_in_module(DEFAULT_INTEROP),
            "@swc/helpers/_/_interop_require_wildcard" => wrap_in_module(WILDCARD_INTEROP),
            "@swc/helpers/_/_export_star" => wrap_in_module(EXPORTS_ALL),
            _ => return Err(anyhow!("swc helper not found: {}", path)),
        };
        Ok(code)
    }
}

fn wrap_in_module(code: &str) -> String {
    format!("function(module, exports){{  {} }}", code)
}
