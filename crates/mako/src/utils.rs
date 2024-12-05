pub(crate) mod id_helper;
pub mod logger;
#[cfg(feature = "profile")]
pub mod profile_gui;
#[cfg(test)]
pub(crate) mod test_helper;
pub mod thread_pool;
pub mod tokio_runtime;

use std::path::Path;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose;
use base64::Engine;
use cached::proc_macro::cached;
use regex::Regex;

pub fn base64_encode<T: AsRef<[u8]>>(raw: T) -> String {
    general_purpose::STANDARD.encode(raw)
}

pub fn base64_decode(bytes: &[u8]) -> Vec<u8> {
    general_purpose::STANDARD.decode(bytes).unwrap()
}

pub fn url_safe_base64_encode<T: AsRef<[u8]>>(raw: T) -> String {
    general_purpose::URL_SAFE.encode(raw)
}

pub fn url_safe_base64_decode(bytes: &[u8]) -> Vec<u8> {
    general_purpose::URL_SAFE.decode(bytes).unwrap()
}

pub trait ParseRegex {
    fn parse_into_regex(&self) -> Result<Option<Regex>>;
}

impl ParseRegex for Option<String> {
    fn parse_into_regex(&self) -> Result<Option<Regex>> {
        self.as_ref().map_or(Ok(None), |v| {
            Regex::new(v)
                .map(Some)
                .map_err(|_| anyhow!("Config Error invalid regex: {}", v))
        })
    }
}

pub fn process_req_url(public_path: &str, req_url: &str) -> Result<String> {
    let public_path = format!("/{}/", public_path.trim_matches('/'));
    if req_url.starts_with(&public_path) {
        return Ok(req_url[public_path.len() - 1..].to_string());
    }
    Ok(req_url.to_string())
}

pub(crate) fn get_app_info(root: &Path) -> (Option<String>, Option<String>) {
    let pkg_json_path = root.join("package.json");

    if pkg_json_path.exists() {
        let pkg_json = std::fs::read_to_string(pkg_json_path).unwrap();
        let pkg_json: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();

        (
            pkg_json
                .get("name")
                .map(|name| name.as_str().unwrap().to_string()),
            pkg_json
                .get("version")
                .map(|name| name.as_str().unwrap().to_string()),
        )
    } else {
        (None, None)
    }
}

#[cached(key = "String", convert = r#"{ re.to_string() }"#)]
pub fn create_cached_regex(re: &str) -> Regex {
    Regex::new(re).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_process_req_url() {
        assert_eq!(
            process_req_url("/public/", "/public/index.html").unwrap(),
            "/index.html"
        );
        assert_eq!(
            process_req_url("/public/foo/", "/public/foo/index.html").unwrap(),
            "/index.html"
        );
        assert_eq!(process_req_url("/", "/index.html").unwrap(), "/index.html");
        assert_eq!(
            process_req_url("/#/", "/#/index.html").unwrap(),
            "/index.html"
        );
    }
}
