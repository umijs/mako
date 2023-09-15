use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::compiler::Context;
use crate::load::{Content, LoadError};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct LessPlugin {}

impl Plugin for LessPlugin {
    fn name(&self) -> &str {
        "less"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "less") {
            // compile less to css
            let mut cmd = Command::new("npx");
            cmd.current_dir(context.root.clone());
            let theme = context.config.less.theme.clone();
            let vars = theme
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
            cmd.args([
                "lessc",
                "--js",
                format!("--modify-var={}", vars).as_str(),
                &param.path,
            ]);

            let output = match cmd.output() {
                Ok(output) => output,
                Err(reason) => {
                    return Err(anyhow!(LoadError::CompileLessError {
                        path: param.path.to_string(),
                        reason: reason.to_string(),
                    }));
                }
            };
            if !output.status.success() {
                let mut reason = String::from_utf8_lossy(&output.stderr).to_string();
                if reason.contains("could not determine executable to run") {
                    reason = "lessc is not found, please install less dependency".to_string();
                }
                return Err(anyhow!(LoadError::CompileLessError {
                    path: param.path.to_string(),
                    reason,
                }));
            }
            let css_content = String::from_utf8_lossy(&output.stdout);
            return Ok(Some(Content::Css(css_content.into())));
        }
        Ok(None)
    }
}
