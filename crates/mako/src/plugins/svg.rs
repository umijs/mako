use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::compiler::Context;
use crate::load::{handle_asset, read_content, Content, LoadError};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct SVGPlugin {}

impl Plugin for SVGPlugin {
    fn name(&self) -> &str {
        "svg"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "svg") {
            let code = read_content(param.path.as_str())?;
            let transform_code = svgr_rs::transform(
                code,
                svgr_rs::Config {
                    named_export: "ReactComponent".to_string(),
                    export_type: Some(svgr_rs::ExportType::Named),
                    ..Default::default()
                },
                svgr_rs::State {
                    ..Default::default()
                },
            );
            // todo: return result<string, error> rather than result<string, string>
            // need svgr-rs to improve
            let svgr_code = match transform_code {
                Ok(res) => res,
                Err(reason) => {
                    return Err(anyhow!(LoadError::ToSvgrError {
                        path: param.path.to_string(),
                        reason,
                    }));
                }
            };
            let default_svg = handle_asset(context, param.path.as_str(), true)?;
            return Ok(Some(Content::Js(format!(
                "{}\nexport default {};",
                svgr_code, default_svg
            ))));
        }
        Ok(None)
    }
}
