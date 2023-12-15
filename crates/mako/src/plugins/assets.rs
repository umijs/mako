use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};

use crate::compiler::Context;
use crate::load::{handle_asset, Content, LoadError};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct AssetsPlugin {}

impl Plugin for AssetsPlugin {
    fn name(&self) -> &str {
        "assets"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if param.task.is_match(vec!["sass", "scss", "stylus"]) {
            return Err(anyhow!(LoadError::UnsupportedExtName {
                ext_name: param.task.ext_name.as_ref().unwrap().to_string(),
                path: param.task.path.clone(),
            }));
        }

        if Path::new(&param.task.request.path).is_file() {
            let asset_content = handle_asset(context, param.task.request.path.as_str(), true)?;
            return Ok(Some(Content::Js(format!(
                "module.exports = {};",
                asset_content
            ))));
        }

        Ok(None)
    }
}
