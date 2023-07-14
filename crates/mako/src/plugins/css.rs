use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct CSSPlugin {}

impl Plugin for CSSPlugin {
    fn name(&self) -> &str {
        "css"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "css") {
            return Ok(Some(Content::Css(read_content(param.path.as_str())?)));
        }
        Ok(None)
    }
}
