use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::lazy_static::lazy_static;
use mako_core::mdxjs::{compile, Options as MdxOptions};
use mako_core::serde_xml_rs::from_str as from_xml_str;
use mako_core::serde_yaml::{from_str as from_yaml_str, Value as YamlValue};
use mako_core::svgr_rs;
use mako_core::thiserror::Error;
use mako_core::toml::{from_str as from_toml_str, Value as TomlValue};
use mako_core::tracing::debug;

use crate::ast_2::file::{Content, File};
use crate::compiler::Context;
use crate::config::Mode;
use crate::plugin::PluginLoadParam;

#[derive(Debug, Error)]
enum LoadError {
    #[error("Unsupported ext name: {ext_name:?} in {path:?}")]
    UnsupportedExtName { ext_name: String, path: String },
    #[error("File not found: {path:?}")]
    FileNotFound { path: String },
    #[error("Read file size error: {path:?}")]
    ReadFileSizeError { path: String },
    #[error("To svgr error: {path:?}, reason: {reason:?}")]
    ToSvgrError { path: String, reason: String },
    #[error("Compile md error: {path:?}, reason: {reason:?}")]
    CompileMdError { path: String, reason: String },
}

lazy_static! {
    static ref JS_EXTENSIONS: Vec<&'static str> = vec!["js", "jsx", "ts", "tsx", "cjs", "mjs"];
    static ref CSS_EXTENSIONS: Vec<&'static str> = vec!["css"];
    static ref JSON_EXTENSIONS: Vec<&'static str> = vec!["json", "json5"];
    static ref YAML_EXTENSIONS: Vec<&'static str> = vec!["yaml", "yml"];
    static ref XML_EXTENSIONS: Vec<&'static str> = vec!["xml"];
    static ref WASM_EXTENSIONS: Vec<&'static str> = vec!["wasm"];
    static ref TOML_EXTENSIONS: Vec<&'static str> = vec!["toml"];
    static ref SVG_EXTENSIONS: Vec<&'static str> = vec!["svg"];
    static ref MD_EXTENSIONS: Vec<&'static str> = vec!["md", "mdx"];
    static ref UNSUPPORTED_EXTENSIONS: Vec<&'static str> = vec!["sass", "scss", "stylus"];
    static ref SVGR_NAMED_EXPORT: String = r#"ReactComponent"#.to_string();
}

pub struct Load {}

impl Load {
    pub fn load(file: &File, context: Arc<Context>) -> Result<Content> {
        mako_core::mako_profile_function!(file.path.to_str());
        debug!("load: {:?}", file);

        // plugin first
        let content: Option<Content> = context
            .plugin_driver
            .load(&PluginLoadParam { file }, &context)?;

        if let Some(content) = content {
            return Ok(content);
        }

        // file exists check must after virtual modules handling
        let path_without_search = PathBuf::from(file.pathname.clone());
        if !path_without_search.exists() || !path_without_search.is_file() {
            return Err(anyhow!(LoadError::FileNotFound {
                path: file.path.to_string_lossy().to_string(),
            }));
        }

        // unsupported
        if UNSUPPORTED_EXTENSIONS.contains(&file.extname.as_str()) {
            return Err(anyhow!(LoadError::UnsupportedExtName {
                ext_name: file.extname.clone(),
                path: file.path.to_string_lossy().to_string(),
            }));
        }

        // ?raw
        if file.has_param("raw") {
            let content = FileSystem::read_file(&path_without_search)?;
            let content = serde_json::to_string(&content)?;
            return Ok(Content::Js(format!("module.exports = {}", content)));
        }

        // js
        if JS_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            return Ok(Content::Js(content));
        }

        // css
        if CSS_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            return Ok(Content::Css(content));
        }

        // md & mdx
        if MD_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            let options = MdxOptions {
                development: matches!(context.config.mode, Mode::Development),
                ..Default::default()
            };
            let content = match compile(&content, &options) {
                Ok(js_string) => js_string,
                Err(reason) => {
                    return Err(anyhow!(LoadError::CompileMdError {
                        path: file.path.to_string_lossy().to_string(),
                        reason,
                    }));
                }
            };
            return Ok(Content::Js(content));
        }

        // svg
        // TODO: Not all svg files need to be converted to React Component, unnecessary performance consumption here
        if SVG_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            let svgr_transformed = svgr_rs::transform(
                content,
                svgr_rs::Config {
                    named_export: SVGR_NAMED_EXPORT.to_string(),
                    export_type: Some(svgr_rs::ExportType::Named),
                    ..Default::default()
                },
                svgr_rs::State {
                    ..Default::default()
                },
            )
            .map_err(|err| LoadError::ToSvgrError {
                path: file.path.to_string_lossy().to_string(),
                reason: err.to_string(),
            })?;
            let asset_path = Self::handle_asset(file, true, context.clone())?;
            return Ok(Content::Js(format!(
                "{}\nexport default {};",
                svgr_transformed, asset_path
            )));
        }

        // toml
        if TOML_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            let content = from_toml_str::<TomlValue>(&content)?;
            let content = serde_json::to_string(&content)?;
            return Ok(Content::Js(format!("module.exports = {}", content)));
        }

        // wasm
        if WASM_EXTENSIONS.contains(&file.extname.as_str()) {
            let final_file_name = format!(
                "{}.{}.{}",
                file.get_file_stem(),
                file.get_content_hash()?,
                file.extname
            );
            context.emit_assets(
                file.path.to_string_lossy().to_string(),
                final_file_name.clone(),
            );
            return Ok(Content::Js(format!(
                "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                final_file_name
            )));
        }

        // xml
        if XML_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            let content = from_xml_str::<serde_json::Value>(&content)?;
            let content = serde_json::to_string(&content)?;
            return Ok(Content::Js(format!("module.exports = {}", content)));
        }

        // yaml
        if YAML_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            let content = from_yaml_str::<YamlValue>(&content)?;
            let content = serde_json::to_string(&content)?;
            return Ok(Content::Js(format!("module.exports = {}", content)));
        }

        // json
        // TODO: json5 should be more complex
        if JSON_EXTENSIONS.contains(&file.extname.as_str()) {
            let content = FileSystem::read_file(&path_without_search)?;
            return Ok(Content::Js(format!("module.exports = {}", content)));
        }

        // assets
        let asset_path = Self::handle_asset(file, true, context.clone())?;
        Ok(Content::Js(format!("module.exports = {};", asset_path)))
    }

    pub fn handle_asset(
        file: &File,
        inject_public_path: bool,
        context: Arc<Context>,
    ) -> Result<String> {
        let path = file.path.to_string_lossy().to_string();
        let file_size = file
            .get_file_size()
            .map_err(|_| LoadError::ReadFileSizeError { path })?;
        let emit_assets = || -> Result<String> {
            let final_file_name = Self::emit_asset(file, context.clone());
            if inject_public_path {
                Ok(format!("`${{require.publicPath}}{}`", final_file_name))
            } else {
                Ok(final_file_name)
            }
        };
        if file_size > context.config.inline_limit.try_into().unwrap() {
            emit_assets()
        } else {
            let base64_result = file.get_base64();
            match base64_result {
                Ok(base64) => {
                    // TODO: why add "" wrapper here?
                    // should have better way to handle this
                    if inject_public_path {
                        Ok(format!("\"{}\"", base64))
                    } else {
                        Ok(base64)
                    }
                }
                Err(_) => emit_assets(),
            }
        }
    }

    pub fn emit_asset(file: &File, context: Arc<Context>) -> String {
        let path = file.path.to_string_lossy().to_string();
        let final_file_name = format!(
            "{}.{}.{}",
            file.get_file_stem(),
            file.get_content_hash().unwrap(),
            file.extname
        );
        context.emit_assets(path, final_file_name.clone());
        final_file_name
    }
}

struct FileSystem {}

impl FileSystem {
    fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
        let mut file = std::fs::File::open(path.as_ref())?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf).to_string())
    }
}
