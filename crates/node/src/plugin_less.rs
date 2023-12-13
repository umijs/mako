use std::sync::{mpsc, Arc};

use crate::{threadsafe_function, ReadMessage};

pub struct LessPlugin {
    pub on_compile_less: threadsafe_function::ThreadsafeFunction<ReadMessage>,
}
use cached::proc_macro::cached;
use mako::compiler::Context;
use mako::load::{read_content, Content};
use mako::plugin::{Plugin, PluginLoadParam};
use mako_core::{anyhow, md5};

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{}-{}", path, md5_hash(_content)) }"#
)]
fn compile_less(
    path: &str,
    _content: &str,
    on_compile_less: &threadsafe_function::ThreadsafeFunction<ReadMessage>,
) -> napi::Result<String> {
    let (tx, rx) = mpsc::channel::<napi::Result<String>>();
    on_compile_less.call(
        ReadMessage {
            message: path.to_string(),
            tx,
        },
        threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
    );
    rx.recv()
        .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))
}

impl Plugin for LessPlugin {
    fn name(&self) -> &str {
        "less"
    }

    fn load(
        &self,
        param: &PluginLoadParam,
        _context: &Arc<Context>,
    ) -> anyhow::Result<Option<Content>> {
        if param.task.is_match(vec!["less"]) {
            let content = read_content(param.task.request.path.as_str())?;
            let content = compile_less(
                param.task.request.path.as_str(),
                &content,
                &self.on_compile_less,
            )?;
            return Ok(Some(Content::Css(content)));
        }
        Ok(None)
    }
}

fn md5_hash<T: AsRef<[u8]>>(content: T) -> String {
    let digest = md5::compute(content);
    format!("{:x}", digest)
}
