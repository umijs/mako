use std::sync::Arc;

use anyhow::Result;
use mdxjs::{compile, Options};

use crate::compiler::Context;
use crate::config::Mode;
use crate::load::{read_content, Content};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct MdPlugin {}

impl Plugin for MdPlugin {
    fn name(&self) -> &str {
        "md"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if context.config.mdx && matches!(param.ext_name.as_str(), "md" | "mdx") {
            let md_string = read_content(param.path.as_str())?;
            let options = Options {
                development: matches!(context.config.mode, Mode::Development),
                ..Default::default()
            };
            let js_string = compile(&md_string, &options).unwrap();
            return Ok(Some(Content::Js(js_string)));
        }
        Ok(None)
    }
}
