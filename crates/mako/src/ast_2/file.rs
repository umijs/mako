use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::base64::alphabet::STANDARD;
use mako_core::base64::{engine, Engine};
use mako_core::lazy_static::lazy_static;
use mako_core::pathdiff::diff_paths;
use mako_core::thiserror::Error;
use mako_core::{md5, mime_guess};

use crate::compiler::Context;

#[derive(Debug, Clone)]
pub enum Content {
    Js(String),
    Css(String),
    // Assets(Asset),
}

#[derive(Debug, Error)]
enum FileError {
    #[error("To base64 error: {path:?}")]
    ToBase64Error { path: String },
}

#[derive(Debug, Clone)]
pub struct File {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub extname: String,
    pub content: Option<Content>,
    pub is_under_node_modules: bool,
    pub is_css_modules: bool,
    pub is_virtual: bool,
    pub is_entry: bool,
    pub pathname: String,
    pub search: String,
    pub params: Vec<(String, String)>,
}

impl Default for File {
    fn default() -> Self {
        File {
            path: PathBuf::new(),
            relative_path: PathBuf::new(),
            extname: "".to_string(),
            content: None,
            is_under_node_modules: false,
            is_css_modules: false,
            is_virtual: false,
            is_entry: false,
            pathname: "".to_string(),
            search: "".to_string(),
            params: vec![],
        }
    }
}

// e.g.
lazy_static! {
    static ref VIRTUAL: String = "virtual:".to_string();
}

impl File {
    pub fn new(path: String, context: Arc<Context>) -> Self {
        let is_virtual = path.starts_with(&*VIRTUAL);
        if is_virtual {
            let path = PathBuf::from(path);
            return File {
                path: path.clone(),
                relative_path: path,
                is_virtual,
                ..Default::default()
            };
        } else {
            let path = PathBuf::from(path);
            let relative_path = diff_paths(&path, &context.root).unwrap_or(path.clone());
            let under_node_modules = path.to_string_lossy().contains("node_modules");
            let extname = path
                .extension()
                .map(|ext| ext.to_string_lossy().to_string())
                .unwrap_or_default();
            let (pathname, search, params) = parse_path(&path.to_string_lossy()).unwrap();
            File {
                is_virtual,
                path,
                relative_path,
                extname,
                is_under_node_modules: under_node_modules,
                pathname,
                search,
                params,
                ..Default::default()
            }
        }
    }

    pub fn new_entry(path: String, context: Arc<Context>) -> Self {
        let mut file = File::new(path, context);
        file.is_entry = true;
        file
    }

    pub fn set_content(&mut self, content: Content) {
        self.content = Some(content);
    }

    pub fn get_content_raw(&self) -> String {
        match &self.content {
            Some(Content::Js(content)) | Some(Content::Css(content)) => content.clone(),
            None => "".to_string(),
        }
    }

    pub fn get_file_stem(&self) -> String {
        self.relative_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn get_file_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.path)?;
        Ok(metadata.len())
    }

    pub fn get_base64(&self) -> Result<String> {
        let content = std::fs::read(&self.path)?;
        let engine = engine::GeneralPurpose::new(&STANDARD, engine::general_purpose::PAD);
        let content = engine.encode(content);
        let guess = mime_guess::from_path(&self.path);
        if let Some(mime) = guess.first() {
            Ok(format!(
                "data:{};base64,{}",
                mime,
                content.replace("\r\n", "")
            ))
        } else {
            Err(anyhow!(FileError::ToBase64Error {
                path: self.path.to_string_lossy().to_string(),
            }))
        }
    }

    pub fn get_content_hash(&self) -> Result<String> {
        let file = std::fs::File::open(&self.path)?;
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
        let hash = format!("{:x}", digest);
        Ok(hash[0..8].to_string())
    }

    pub fn has_param(&self, key: &str) -> bool {
        self.params.iter().any(|(k, _)| k == key)
    }
}

fn parse_path(path: &str) -> Result<(String, String, Vec<(String, String)>)> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    let search = if query == "" {
        "".to_string()
    } else {
        format!("?{}", query)
    };
    Ok((path.to_string(), search, query_vec))
}
