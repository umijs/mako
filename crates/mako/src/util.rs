use std::path::PathBuf;

use mako_core::anyhow::{anyhow, Result};
use mako_core::base64::engine::general_purpose;
use mako_core::base64::Engine;
use mako_core::merge_source_map::sourcemap::SourceMap;
use mako_core::merge_source_map::{merge, MergeOptions};
use mako_core::pathdiff::diff_paths;
use mako_core::regex::Regex;

pub fn base64_encode<T: AsRef<[u8]>>(raw: T) -> String {
    general_purpose::STANDARD.encode(raw)
}

pub fn base64_decode(bytes: &[u8]) -> Vec<u8> {
    general_purpose::STANDARD.decode(bytes).unwrap()
}

pub fn merge_source_map(source_map_chain: Vec<Vec<u8>>, root: PathBuf) -> Vec<u8> {
    let source_map_chain = source_map_chain
        .iter()
        .map(|s| SourceMap::from_slice(s).unwrap())
        .collect::<Vec<_>>();

    let merged = merge(
        source_map_chain,
        MergeOptions {
            source_replacer: Some(Box::new(move |src| {
                diff_paths(src, &root)
                    .unwrap_or(src.into())
                    .to_string_lossy()
                    .to_string()
            })),
        },
    );

    let mut buf = vec![];
    merged.to_writer(&mut buf).unwrap();
    buf
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
