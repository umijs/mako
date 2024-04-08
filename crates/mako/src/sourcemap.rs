use std::path::PathBuf;

use mako_core::merge_source_map::sourcemap::SourceMap as MergeSourceMap;
use mako_core::merge_source_map::{merge, MergeOptions};
use mako_core::pathdiff::diff_paths;
use mako_core::swc_common::source_map::SourceMapGenConfig;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{BytePos, FileName, LineCol, SourceMap};
use swc_core::base::sourcemap;
pub struct SwcSourceMapGenConfig;

impl SourceMapGenConfig for SwcSourceMapGenConfig {
    fn file_name_to_source(&self, f: &FileName) -> String {
        f.to_string()
    }

    /// 生成 sourceContents
    fn inline_sources_content(&self, _f: &FileName) -> bool {
        true
    }
}

pub fn build_source_map_to_buf(mappings: &[(BytePos, LineCol)], cm: &Lrc<SourceMap>) -> Vec<u8> {
    let mut src_buf = vec![];

    let sm = build_source_map(mappings, cm);

    sm.to_writer(&mut src_buf).unwrap();

    src_buf
}

fn build_source_map(mappings: &[(BytePos, LineCol)], cm: &Lrc<SourceMap>) -> sourcemap::SourceMap {
    let config = SwcSourceMapGenConfig;

    cm.build_source_map_with_config(mappings, None, config)
}

pub fn merge_source_map(source_map_chain: Vec<Vec<u8>>, root: PathBuf) -> Vec<u8> {
    let source_map_chain = source_map_chain
        .iter()
        .map(|s| MergeSourceMap::from_slice(s).unwrap())
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
