use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cached::proc_macro::cached;
use tracing::debug;

use crate::compiler::Context;
use crate::load::{read_content, Content, LoadError};
use crate::plugin::{Plugin, PluginLoadParam};

pub struct LessPlugin {}

impl Plugin for LessPlugin {
    fn name(&self) -> &str {
        "less"
    }

    fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "less") {
            let content = read_content(param.path.as_str())?;
            let css_content = compile_less(param, content.as_str(), context)?;
            return Ok(Some(Content::Css(css_content)));
        }
        Ok(None)
    }
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{}-{}", param.path, _content) }"#
)]
fn compile_less(param: &PluginLoadParam, _content: &str, context: &Arc<Context>) -> Result<String> {
    let lessc_path = context.config.less.lessc_path.clone();
    let installed_node = context.root.join("node_modules/.bin/node");
    let mut cmd = if lessc_path.is_empty() {
        Command::new("npx")
    } else {
        // use user specified node first
        // tnpm will install node to node_modules/.bin/node
        if installed_node.exists() {
            Command::new(installed_node)
        } else {
            Command::new("node")
        }
    };
    cmd.current_dir(context.root.clone());
    let theme = context.config.less.theme.clone();
    let mut args = Vec::from([]);
    if lessc_path.is_empty() {
        args.push("lessc".to_string());
    } else {
        args.push(lessc_path);
    }
    args.push("--js".to_string());
    if !theme.is_empty() {
        theme.iter().for_each(|(k, v)| {
            args.push(format!("--modify-var={}=\'{}\'", k, v));
        });
    }
    args.push(param.path.to_string());
    cmd.args(args);
    debug!("compile less: {:?}", cmd);

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
    Ok(css_content.into())
}
