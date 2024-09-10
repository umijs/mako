pub mod logger;
#[cfg(feature = "profile")]
pub mod profile_gui;
#[cfg(test)]
pub(crate) mod test_helper;
pub(crate) mod thread_pool;
pub mod tokio_runtime;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose;
use base64::Engine;
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
