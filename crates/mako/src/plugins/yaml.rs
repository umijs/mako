use std::sync::Arc;

use anyhow::Result;
use mako_core::serde_yaml::{from_str as from_yaml_str, Value as YamlValue};

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct YAMLPlugin {}

impl Plugin for YAMLPlugin {
    fn name(&self) -> &str {
        "yaml"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "yaml") {
            let yaml_string = read_content(param.path.as_str())?;
            let yaml_value = from_yaml_str::<YamlValue>(&yaml_string)?;
            let json_string = serde_json::to_string(&yaml_value)?;
            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                json_string
            ))));
        }
        Ok(None)
    }
}
