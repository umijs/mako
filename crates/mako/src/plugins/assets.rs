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
        if matches!(param.ext_name.as_str(), "sass" | "scss" | "stylus") {
            return Err(anyhow!(LoadError::UnsupportedExtName {
                ext_name: param.ext_name.clone(),
                path: param.path.clone(),
            }));
        }

        let asset_content = handle_asset(context, param.path.as_str(), true)?;
        Ok(Some(Content::Js(format!(
            "module.exports = {};",
            asset_content
        ))))
    }
}
