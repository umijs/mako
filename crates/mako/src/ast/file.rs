use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use pathdiff::diff_paths;
use percent_encoding::percent_decode_str;
use regex::Regex;
use thiserror::Error;
use twox_hash::XxHash64;
use url::Url;
use {md5, mime_guess};

use crate::compiler::Context;
use crate::utils::{base64_decode, base64_encode};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Asset {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JsContent {
    pub is_jsx: bool,
    pub content: String,
}

impl Default for JsContent {
    fn default() -> Self {
        JsContent {
            is_jsx: false,
            content: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Content {
    Js(JsContent),
    Css(String),
    // TODO: unify the assets handler
    // it's used in minifish plugin(bundless mode) only
    // and bundle mode will emit assets to context.assets_info
    Assets(Asset),
}

#[derive(Debug, Error)]
enum FileError {
    #[error("To base64 error: {path:?}")]
    ToBase64Error { path: String },
}

#[derive(Debug, Clone, Eq)]
pub struct File {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub extname: String,
    pub content: Option<Content>,
    pub is_under_node_modules: bool,
    pub is_css_modules: bool,
    pub is_virtual: bool,
    pub is_entry: bool,
    pub pathname: PathBuf,
    pub search: String,
    pub params: Vec<(String, String)>,
    pub fragment: Option<String>,
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
            pathname: PathBuf::new(),
            search: "".to_string(),
            params: vec![],
            fragment: None,
        }
    }
}

impl Hash for File {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.pathname.to_string_lossy().as_bytes());
    }
}

impl PartialEq for File {
    fn eq(&self, other: &Self) -> bool {
        self.pathname.eq(&other.pathname)
    }
}

const VIRTUAL: &str = "virtual:";

fn css_source_map_regex() -> &'static Regex {
    static CSS_SOURCE_MAP_REGEXP: OnceLock<Regex> = OnceLock::new();

    CSS_SOURCE_MAP_REGEXP.get_or_init(|| {
        Regex::new(r"/\*# sourceMappingURL=data:application/json;base64,(.*?) \*/").unwrap()
    })
}

impl File {
    pub fn new(path: String, context: Arc<Context>) -> Self {
        let path = PathBuf::from(path);
        // if path exists, it has no search and fragment
        // support ./a#b.ts when a#b.ts is a real file
        // e.g. https://unpkg.com/browse/es5-ext@0.10.64/string/
        let (pathname, search, params, fragment) = if path.exists() {
            (
                path.to_string_lossy().to_string(),
                "".to_string(),
                vec![],
                None,
            )
        } else {
            parse_path(&path.to_string_lossy()).unwrap()
        };
        let pathname = PathBuf::from(pathname);
        let is_virtual = path.starts_with(VIRTUAL) ||
            // TODO: remove this specific logic
            params.iter().any(|(k, _)| k == "asmodule");
        let is_under_node_modules = path.to_string_lossy().contains("node_modules");
        let extname = pathname
            .clone()
            .extension()
            .map(|ext| ext.to_string_lossy().to_string())
            .unwrap_or_default();
        if is_virtual {
            File {
                path: path.clone(),
                relative_path: path,
                is_virtual,
                pathname,
                search,
                params,
                fragment,
                is_under_node_modules,
                extname,
                ..Default::default()
            }
        } else {
            let relative_path = diff_paths(&path, &context.root).unwrap_or(path.clone());
            let relative_path = PathBuf::from(win_path(relative_path.to_str().unwrap()));
            File {
                is_virtual,
                path,
                relative_path,
                extname,
                is_under_node_modules,
                pathname,
                search,
                params,
                ..Default::default()
            }
        }
    }

    #[allow(dead_code)]
    pub fn with_content(path: String, content: Content, context: Arc<Context>) -> Self {
        let mut file = File::new(path, context);
        file.content = Some(content);
        file
    }

    pub fn is_css(&self) -> bool {
        self.content.is_some() && matches!(self.content.as_ref().unwrap(), Content::Css(_))
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
            Some(Content::Js(JsContent { content, .. })) | Some(Content::Css(content)) => {
                content.clone()
            }
            Some(Content::Assets(asset)) => asset.content.clone(),
            None => "".to_string(),
        }
    }

    pub fn get_raw_hash(&self) -> u64 {
        let mut hasher: XxHash64 = Default::default();
        if let Some(content) = &self.content {
            match content {
                Content::Js(JsContent { content, .. })
                | Content::Css(content)
                | Content::Assets(Asset { content, .. }) => {
                    // hasher.write_u64(init);
                    hasher.write(content.as_bytes());
                    hasher.finish()
                }
            }
        } else {
            0
        }
    }

    pub fn get_file_stem(&self) -> String {
        self.relative_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn get_file_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.pathname)?;
        Ok(metadata.len())
    }

    pub fn get_base64(&self) -> Result<String> {
        let content = std::fs::read(&self.pathname)?;
        let content_base64 = base64_encode(content);
        let guess = mime_guess::from_path(&self.pathname);
        if let Some(mime) = guess.first() {
            Ok(format!(
                "data:{};base64,{}",
                mime,
                content_base64.replace("\r\n", "")
            ))
        } else {
            Err(anyhow!(FileError::ToBase64Error {
                path: self.path.to_string_lossy().to_string(),
            }))
        }
    }

    pub fn get_content_hash(&self) -> Result<String> {
        let file = std::fs::File::open(&self.pathname)?;
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

    pub fn is_content_jsx(&self) -> bool {
        match &self.content {
            Some(Content::Js(JsContent { is_jsx, .. })) => *is_jsx,
            _ => false,
        }
    }

    pub fn has_param(&self, key: &str) -> bool {
        self.params.iter().any(|(k, _)| k == key)
    }

    pub fn param(&self, key: &str) -> Option<String> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    pub fn get_source_map_chain(&self, context: Arc<Context>) -> Vec<Vec<u8>> {
        if context.config.devtool.is_none() {
            return vec![];
        }
        let mut chain = vec![];
        match &self.content {
            Some(Content::Css(content)) => {
                if let Some(captures) = css_source_map_regex().captures(content) {
                    let source_map_base64 = captures.get(1).unwrap().as_str().to_string();
                    chain.push(base64_decode(source_map_base64.as_bytes()));
                }
            }
            // TODO: support js source map chain
            Some(Content::Js(_)) => {}
            _ => {}
        }
        chain
    }

    pub fn path(&self) -> Option<String> {
        let path_string = self.path.to_string_lossy().to_string();
        if path_string.starts_with(VIRTUAL) {
            self.param("path")
        } else {
            Some(path_string)
        }
    }

    pub fn resolve_from(&self, context: &Arc<Context>) -> String {
        self.path().map_or_else(
            || {
                context
                    .root
                    .join(".virtual.root")
                    .to_string_lossy()
                    .to_string()
            },
            |p| p,
        )
    }
}

type PathName = String;
type Search = String;
type Params = Vec<(String, String)>;
type Fragment = Option<String>;

fn has_hash_without_dot(input: &str) -> bool {
    if let Some(pos) = input.find('#') {
        let after_hash = &input[pos + 1..];
        !after_hash.contains('.')
    } else {
        false
    }
}

pub fn win_path(path: &str) -> String {
    #[cfg(target_os = "windows")]
    let path = {
        let prefix = "\\\\?\\";
        let path = path.trim_start_matches(prefix);
        path.replace('\\', "/")
    };
    #[cfg(not(target_os = "windows"))]
    let path = path.to_string();
    path
}

pub fn parse_path(path: &str) -> Result<(PathName, Search, Params, Fragment)> {
    let path = win_path(path);
    if path.contains('?') || has_hash_without_dot(path.as_str()) {
        let (path, search) = if path.contains('?') {
            path.split_once('?').unwrap_or((path.as_str(), ""))
        } else {
            path.split_once('#').unwrap_or((path.as_str(), ""))
        };
        let base = "http://a.com/";
        let base_url = Url::parse(base)?;
        let full_url = base_url.join(format!("?{}", search).as_str())?;
        let fragment = full_url.fragment().map(|s| s.to_string());
        let search = full_url.query().unwrap_or("").to_string();
        let query_vec = full_url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        // dir or filename may contains space or other special characters
        // so we need to decode it, e.g. "a%20b" -> "a b"
        let path = percent_decode_str(path).decode_utf8()?;
        Ok((path.to_string(), search.to_string(), query_vec, fragment))
    } else {
        let path = percent_decode_str(&path).decode_utf8()?;
        Ok((path.to_string(), "".to_string(), vec![], None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abs_path() {
        let f = File::new("/a/b/c".to_string(), Arc::new(Context::default()));
        assert_eq!(f.path(), Some("/a/b/c".to_string()));
    }

    #[test]
    fn test_virtual_file_without_virtual_path() {
        let f = File::new("virtual:/a/b/c".to_string(), Arc::new(Context::default()));
        assert_eq!(f.path(), None);
    }

    #[test]
    fn test_virtual_file_with_virtual_path() {
        let f = File::new(
            "virtual:/a/b/c?path=/root/d.js".to_string(),
            Arc::new(Context::default()),
        );
        assert_eq!(f.path(), Some("/root/d.js".to_string()));
    }

    #[test]
    fn test_parse_path_support_windows() {
        let path = "C:\\a\\b\\c?foo";
        let (path, search, params, fragment) = parse_path(path).unwrap();
        assert_eq!(path, "C:\\a\\b\\c");
        assert_eq!(search, "foo");
        assert_eq!(params, vec![("foo".to_string(), "".to_string())]);
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_parse_path_with_fragment() {
        assert_eq!(parse_path("foo.ts#bar").unwrap().0, "foo.ts");
        assert_eq!(parse_path("foo#bar.ts").unwrap().0, "foo#bar.ts");
    }

    #[test]
    fn test_has_hash_without_dot() {
        assert!(has_hash_without_dot("foo.ts#world"));
        assert!(!has_hash_without_dot("foo#bar.ts"));
        assert!(has_hash_without_dot("#no_dot"));
        assert!(!has_hash_without_dot("no_hash"));
        assert!(!has_hash_without_dot("#.dot_after_hash"));
    }
}
