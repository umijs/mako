use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context as AnyHowContext, Result};
use base64::alphabet::STANDARD;
use base64::{engine, Engine};
use thiserror::Error;
use tracing::debug;

use crate::compiler::Context;
use crate::css_modules::{is_mako_css_modules, MAKO_CSS_MODULES_SUFFIX};

pub struct Asset {
    pub path: String,
    pub content: String,
}

pub enum Content {
    Js(String),
    Css(String),
    Assets(Asset),
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
}

pub fn load(path: &str, context: &Arc<Context>) -> Result<Content> {
    debug!("load: {}", path);
    let path = if is_mako_css_modules(path) {
        path.trim_end_matches(MAKO_CSS_MODULES_SUFFIX)
    } else {
        path
    };
    let exists = Path::new(path).exists();
    if !exists {
        return Err(anyhow!(LoadError::FileNotFound {
            path: path.to_string(),
        }));
    }

    let ext_name = ext_name(path);
    match ext_name {
        Some("js" | "jsx" | "ts" | "tsx" | "cjs" | "mjs") => load_js(path),
        Some("css") => load_css(path),
        Some("json") => load_json(path),
        Some("less" | "sass" | "scss" | "stylus") => Err(anyhow!(LoadError::UnsupportedExtName {
            ext_name: ext_name.unwrap().to_string(),
            path: path.to_string(),
        })),
        _ => load_assets(path, context),
    }
}

fn load_js(path: &str) -> Result<Content> {
    Ok(Content::Js(read_content(path)?))
}

fn load_css(path: &str) -> Result<Content> {
    Ok(Content::Css(read_content(path)?))
}

fn load_json(path: &str) -> Result<Content> {
    Ok(Content::Js(format!(
        "module.exports = {}",
        read_content(path)?
    )))
}

fn load_assets(path: &str, context: &Arc<Context>) -> Result<Content> {
    let file_size = file_size(path).with_context(|| LoadError::ReadFileSizeError {
        path: path.to_string(),
    })?;

    if file_size > context.config.data_url_limit.try_into().unwrap() {
        let final_file_name = content_hash(path)? + "." + ext_name(path).unwrap();
        let path = path.to_string();
        context.emit_assets(path.clone(), final_file_name.clone());
        Ok(Content::Assets(Asset {
            // TODO: improve assets structure
            path,
            content: format!("module.exports = \"{}\"", final_file_name),
        }))
    } else {
        let base64 = to_base64(path).with_context(|| LoadError::ToBase64Error {
            path: path.to_string(),
        })?;
        Ok(Content::Js(format!("export default \"{}\";", base64)))
    }
}

fn read_content(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("read file error: {}", path))
}

fn ext_name(path: &str) -> Option<&str> {
    let ext = Path::new(path).extension();
    if let Some(ext) = ext {
        return ext.to_str();
    }
    None
}

fn file_size(path: &str) -> Result<u64> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len())
}

fn to_base64(path: &str) -> Result<String> {
    let vec = std::fs::read(path)?;
    let engine = engine::GeneralPurpose::new(&STANDARD, engine::general_purpose::PAD);
    let base64 = engine.encode(&vec);
    let file_type = ext_name(path).unwrap();
    Ok(format!(
        "data:image/{};base64,{}",
        file_type,
        base64.replace("\r\n", "")
    ))
}

fn content_hash(file_path: &str) -> Result<String> {
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
