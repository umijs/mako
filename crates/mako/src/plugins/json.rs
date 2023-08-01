use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct JSONPlugin {}

impl Plugin for JSONPlugin {
    fn name(&self) -> &str {
        "json"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        // TODO: json5 应该没这么简单

        if matches!(param.ext_name.as_str(), "json" | "json5") {
            dbg!(&param);

            let root = _context.root.clone();
            let to: PathBuf = param.path.clone().into();

            let relative = to
                .strip_prefix(root)
                .unwrap_or_else(|_| panic!("{:?} not under project root", to))
                .to_str()
                .unwrap();

            return match _context.config.minifish_map.get(relative) {
                Some(js_content) => Ok(Some(Content::Js(js_content.to_string()))),
                None => Ok(Some(Content::Js(format!(
                    "module.exports = {}",
                    read_content(param.path.as_str())?
                )))),
            };
        }
        Ok(None)
    }
}

pub struct APPXJSONPlugin {}

impl Plugin for APPXJSONPlugin {
    fn name(&self) -> &str {
        "appx_json"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        // TODO: json5 应该没这么简单

        dbg!(&param.path);

        if matches!(param.ext_name.as_str(), "json" | "json5") {
            return Ok(Some(Content::Js(format!(
                "module.exports = {}",
                read_content(param.path.as_str())?
            ))));
        }
        Ok(None)
    }
}
