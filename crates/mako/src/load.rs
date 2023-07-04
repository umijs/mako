use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context as AnyHowContext, Result};
use base64::alphabet::STANDARD;
use base64::{engine, Engine};
use serde_xml_rs::from_str as from_xml_str;
use serde_yaml::{from_str as from_yaml_str, Value as YamlValue};
use thiserror::Error;
use toml::{from_str as from_toml_str, Value as TomlValue};
use tracing::debug;

use crate::compiler::Context;
use crate::config::Mode;
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

pub fn load(path: &str, is_entry: bool, context: &Arc<Context>) -> Result<Content> {
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
        Some("js" | "jsx" | "ts" | "tsx" | "cjs" | "mjs") => {
            let mut content = read_content(path)?;
            // TODO: use array entry instead
            if is_entry && context.config.hmr && context.config.mode == Mode::Development {
                let port = &context.config.hmr_port.to_string();
                let host = &context.config.hmr_host.to_string();
                let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
                content = format!(
                    "{}\n{}\n",
                    content,
                    include_str!("runtime/runtime_hmr_entry.js")
                )
                .replace("__PORT__", port)
                .replace("__HOST__", host);
            }
            Ok(Content::Js(content))
        }
        Some("css") => load_css(path),
        Some("json" | "json5") => load_json(path),
        Some("toml") => load_toml(path),
        Some("yaml") => load_yaml(path),
        Some("xml") => load_xml(path),
        Some("wasm") => load_wasm(path, context),
        Some("svg") => load_svg(path),
        Some("less" | "sass" | "scss" | "stylus") => Err(anyhow!(LoadError::UnsupportedExtName {
            ext_name: ext_name.unwrap().to_string(),
            path: path.to_string(),
        })),
        _ => load_assets(path, context),
    }
}

#[allow(dead_code)]
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

fn load_toml(path: &str) -> Result<Content> {
    let toml_string = read_content(path)?;
    let toml_value = from_toml_str::<TomlValue>(&toml_string)?;
    let json_string = serde_json::to_string(&toml_value)?;
    Ok(Content::Js(format!("module.exports = {}", json_string)))
}

fn load_yaml(path: &str) -> Result<Content> {
    let yaml_string = read_content(path)?;
    let yaml_value = from_yaml_str::<YamlValue>(&yaml_string)?;
    let json_string = serde_json::to_string(&yaml_value)?;
    Ok(Content::Js(format!("module.exports = {}", json_string)))
}

fn load_xml(path: &str) -> Result<Content> {
    let xml_string = read_content(path)?;
    let xml_value = from_xml_str::<serde_json::Value>(&xml_string)?;
    let json_string = serde_json::to_string(&xml_value)?;
    Ok(Content::Js(format!("module.exports = {}", json_string)))
}

fn load_wasm(path: &str, context: &Arc<Context>) -> Result<Content> {
    let file_size = file_size(path).with_context(|| LoadError::ReadFileSizeError {
        path: path.to_string(),
    })?;

    if file_size > context.config.inline_limit.try_into().unwrap() {
        let final_file_name = content_hash(path)? + "." + ext_name(path).unwrap();
        context.emit_assets(path.to_string(), final_file_name.clone());

        Ok(Content::Assets(Asset {
            path: path.to_string(),
            content: format!(
                "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                final_file_name
            ),
        }))
    } else {
        let raw = to_base64(path)?.replace("data:application/wasm;base64,", "");
        Ok(Content::Js(format!(
            "
                const raw = globalThis.atob('{raw}');
                const rawLength = raw.length;
                const buf = new Uint8Array(new ArrayBuffer(rawLength));
                for (let i = 0; i < rawLength; i++) {{
                    buf[i] = raw.charCodeAt(i);
                }}
                module.exports = WebAssembly.instantiate(buf).then(({{ instance }}) => instance.exports);
            "
        )))
    }
}

fn load_svg(path: &str) -> Result<Content> {
    let code = read_content(path)?;
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
    // todo: 1.return result<string, error> rather than result<string, string>
    // todo: 2.transform class to className
    // have submit issues https://github.com/svg-rust/svgr-rs/issues/21
    let svgr_code = match transform_code {
        Ok(res) => res,
        Err(res) => res,
    };

    // todo: now all svg will base64
    // will improve the case - large file, after assets structure improved which metioned in load_assets
    let base64 = to_base64(path).with_context(|| LoadError::ToBase64Error {
        path: path.to_string(),
    })?;

    Ok(Content::Js(format!(
        "{}\nexport default \"{}\";",
        svgr_code, base64
    )))
}

fn load_assets(path: &str, context: &Arc<Context>) -> Result<Content> {
    let file_size = file_size(path).with_context(|| LoadError::ReadFileSizeError {
        path: path.to_string(),
    })?;

    if file_size > context.config.inline_limit.try_into().unwrap() {
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
