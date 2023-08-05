use std::fs;
use std::hash::Hasher;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context as AnyHowContext, Result};
use base64::alphabet::STANDARD;
use base64::{engine, Engine};
use thiserror::Error;
use tracing::debug;
use twox_hash::XxHash64;

use crate::build::FileRequest;
use crate::compiler::Context;
use crate::plugin::PluginLoadParam;

pub struct Asset {
    pub path: String,
    pub content: String,
}

pub enum Content {
    Js(String),
    Css(String),
    #[allow(dead_code)]
    Assets(Asset),
}

impl Content {
    pub fn raw_hash(&self) -> u64 {
        let mut hasher: XxHash64 = Default::default();
        match self {
            Content::Js(content)
            | Content::Css(content)
            | Content::Assets(Asset { content, .. }) => {
                hasher.write(content.as_bytes());
                hasher.finish()
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Unsupported ext name: {ext_name:?} in {path:?}")]
    UnsupportedExtName { ext_name: String, path: String },
    #[error("To base64 error: {path:?}")]
    ToBase64Error { path: String },
    #[error("File not found: {path:?}")]
    FileNotFound { path: String },
    #[error("Read file size error: {path:?}")]
    ReadFileSizeError { path: String },
    #[error("To svgr error: {path:?}, reason: {reason:?}")]
    ToSvgrError { path: String, reason: String },
}

pub fn load(request: &FileRequest, is_entry: bool, context: &Arc<Context>) -> Result<Content> {
    debug!("load: {:?}", request);
    let path = &request.path;
    let exists = Path::new(path).exists();
    if !exists {
        return Err(anyhow!(LoadError::FileNotFound {
            path: path.to_string(),
        }));
    }

    let content = context.plugin_driver.load(
        &PluginLoadParam {
            path: path.to_string(),
            is_entry,
            ext_name: ext_name(path).unwrap().to_string(),
        },
        context,
    )?;

    Ok(content.unwrap())
}

// 统一处理各类 asset，将其转为 base64 or 静态资源
pub fn handle_asset<T: AsRef<str>>(context: &Arc<Context>, path: T) -> Result<String> {
    let path_str = path.as_ref();
    let path_string = path_str.to_string();
    let file_size = file_size(path_str).with_context(|| LoadError::ReadFileSizeError {
        path: path_string.clone(),
    })?;

    if file_size > context.config.inline_limit.try_into().unwrap() {
        let final_file_name = content_hash(path_str)? + "." + ext_name(path_str).unwrap();
        context.emit_assets(path_string, final_file_name.clone());
        Ok(final_file_name)
    } else {
        let base64 =
            to_base64(path_str).with_context(|| LoadError::ToBase64Error { path: path_string })?;
        Ok(base64)
    }
}

pub fn read_content<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("read file error: {:?}", path.as_ref()))
}

fn ext_name(path: &str) -> Option<&str> {
    let ext = Path::new(path).extension();
    if let Some(ext) = ext {
        return ext.to_str();
    }
    None
}

pub fn file_size(path: &str) -> Result<u64> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len())
}

fn to_base64(path: &str) -> Result<String> {
    let vec = std::fs::read(path)?;
    let engine = engine::GeneralPurpose::new(&STANDARD, engine::general_purpose::PAD);
    let base64 = engine.encode(vec);
    let guess = mime_guess::from_path(path);
    if let Some(mime) = guess.first() {
        Ok(format!(
            "data:{};base64,{}",
            mime,
            base64.replace("\r\n", "")
        ))
    } else {
        Err(anyhow!(LoadError::ToBase64Error {
            path: path.to_string(),
        }))
    }
}

pub fn content_hash(file_path: &str) -> Result<String> {
    let file = fs::File::open(file_path)?;
    // Find the length of the file
    let len = file.metadata()?.len();
    // Decide on a reasonable buffer size (1MB in this case, fastest will depend on hardware)
    let buf_len = len.min(1_000_000) as usize;
    let mut buf = BufReader::with_capacity(buf_len, file);
    // webpack use md4
    let mut context = md5::Context::new();
    loop {
        // Get a chunk of the file
        let part = buf.fill_buf()?;
        if part.is_empty() {
            break;
        }
        context.consume(part);
        // Tell the buffer that the chunk is consumed
        let part_len = part.len();
        buf.consume(part_len);
    }
    let digest = context.compute();
    Ok(format!("{:x}", digest))
}
