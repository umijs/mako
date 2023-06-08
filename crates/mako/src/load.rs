use base64::{alphabet::STANDARD, engine, Engine};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
};
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

pub fn load(path: &str, context: &Arc<Context>) -> Content {
    debug!("load: {}", path);
    let mut path = path;
    if is_mako_css_modules(path) {
        path = path.trim_end_matches(MAKO_CSS_MODULES_SUFFIX);
    }
    let exists = Path::new(path).exists();
    if !exists {
        panic!("file not found: {}", path);
    }

    let ext_name = ext_name(path);
    match ext_name {
        Some("js" | "jsx" | "ts" | "tsx" | "cjs" | "mjs") => load_js(path),
        Some("css") => load_css(path),
        Some("json") => load_json(path),
        _ => load_assets(path, context),
    }
}

fn load_js(path: &str) -> Content {
    Content::Js(read_content(path))
}

fn load_css(path: &str) -> Content {
    Content::Css(read_content(path))
}

fn load_json(path: &str) -> Content {
    Content::Js(format!("module.exports = {}", read_content(path)))
}

fn load_assets(path: &str, context: &Arc<Context>) -> Content {
    let file_size = file_size(path);
    if file_size.is_err() {
        panic!("read file size error: {}", path);
    }
    let file_size = file_size.unwrap();

    if file_size > context.config.data_url_limit.try_into().unwrap() {
        let final_file_name = content_hash(path).unwrap() + "." + ext_name(path).unwrap();
        let path = path.to_string();
        context.emit_assets(path.clone(), final_file_name.clone());
        Content::Assets(Asset {
            // TODO: improve assets structure
            path,
            content: format!("module.exports = \"{}\"", final_file_name),
        })
    } else {
        let base64 = to_base64(path);
        if base64.is_err() {
            panic!("to base64 error: {}", path);
        }
        let base64 = base64.unwrap();
        Content::Js(format!("export default \"{}\";", base64))
    }
}

fn read_content(path: &str) -> String {
    let content = std::fs::read_to_string(path);
    if content.is_err() {
        panic!("read file error: {}", path);
    }
    content.unwrap()
}

fn ext_name(path: &str) -> Option<&str> {
    let ext = Path::new(path).extension();
    if let Some(ext) = ext {
        return ext.to_str();
    }
    None
}

fn file_size(path: &str) -> anyhow::Result<u64> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len())
}

fn to_base64(path: &str) -> anyhow::Result<String> {
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

fn content_hash(file_path: &str) -> anyhow::Result<String> {
    let file = fs::File::open(file_path).unwrap();
    // Find the length of the file
    let len = file.metadata().unwrap().len();
    // Decide on a reasonable buffer size (1MB in this case, fastest will depend on hardware)
    let buf_len = len.min(1_000_000) as usize;
    let mut buf = BufReader::with_capacity(buf_len, file);
    // webpack use md4
    let mut context = md5::Context::new();
    loop {
        // Get a chunk of the file
        let part = buf.fill_buf().unwrap();
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
