use std::path::PathBuf;

use mako_core::base64::engine::general_purpose;
use mako_core::base64::Engine;
use mako_core::merge_source_map::sourcemap::SourceMap;
use mako_core::merge_source_map::{merge, MergeOptions};
use mako_core::pathdiff::diff_paths;

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
