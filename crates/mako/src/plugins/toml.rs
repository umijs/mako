use std::sync::Arc;

use anyhow::Result;
use mako_core::toml::{from_str as from_toml_str, Value as TomlValue};

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct TOMLPlugin {}

impl Plugin for TOMLPlugin {
    fn name(&self) -> &str {
        "toml"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "toml") {
            let toml_string = read_content(param.path.as_str())?;
            let toml_value = from_toml_str::<TomlValue>(&toml_string)?;
            let json_string = serde_json::to_string(&toml_value)?;
            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                json_string
            ))));
        }
        Ok(None)
    }
}
