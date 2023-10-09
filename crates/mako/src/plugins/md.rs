use std::sync::Arc;

use anyhow::{anyhow, Result};
use mako_core::mdxjs::{compile, Options};

use crate::compiler::Context;
use crate::config::Mode;
use crate::load::{read_content, Content, LoadError};
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
            let js_string = match compile(&md_string, &options) {
                Ok(js_string) => js_string,
                Err(reason) => {
                    return Err(anyhow!(LoadError::CompileMdError {
                        path: param.path.to_string(),
                        reason,
                    }));
                }
            };
            return Ok(Some(Content::Js(js_string)));
        }
        Ok(None)
    }
}
