use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::serde_xml_rs::from_str as from_xml_str;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct XMLPlugin {}

impl Plugin for XMLPlugin {
    fn name(&self) -> &str {
        "xml"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "xml") {
            let xml_string = read_content(param.path.as_str())?;
            let xml_value = from_xml_str::<serde_json::Value>(&xml_string)?;
            let json_string = serde_json::to_string(&xml_value)?;
            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                json_string
            ))));
        }
        Ok(None)
    }
}
