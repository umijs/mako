use std::path::PathBuf;

use merge_source_map::sourcemap::SourceMap as MergeSourceMap;
use merge_source_map::{merge, MergeOptions};
use pathdiff::diff_paths;
use swc_core::base::sourcemap;
use swc_core::common::source_map::SourceMapGenConfig;
use swc_core::common::sync::Lrc;
use swc_core::common::{BytePos, FileName, LineCol, SourceMap};

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
    let sm = build_source_map(mappings, cm);

    let mut src_buf = vec![];

    sm.to_writer(&mut src_buf).unwrap();

    src_buf
}

pub fn build_source_map(
    mappings: &[(BytePos, LineCol)],
    cm: &Lrc<SourceMap>,
) -> sourcemap::SourceMap {
    let config = SwcSourceMapGenConfig;

    cm.build_source_map_with_config(mappings, None, config)
}

// Add this type because the sourcemap::SourceMap type can't be cached,
// there is a RefCell type field in it
#[derive(Clone, Default, Debug)]
pub struct RawSourceMap {
    pub file: Option<String>,
    pub tokens: Vec<sourcemap::RawToken>,
    pub names: Vec<String>,
    pub sources: Vec<String>,
    pub sources_content: Vec<Option<String>>,
}

impl From<sourcemap::SourceMap> for RawSourceMap {
    fn from(sm: sourcemap::SourceMap) -> Self {
        Self {
            file: sm.get_file().map(|f| f.to_owned()),
            tokens: sm.tokens().map(|t| t.get_raw_token()).collect(),
            names: sm.names().map(|n| n.to_owned()).collect(),
            sources: sm.sources().map(|s| s.to_owned()).collect(),
            sources_content: sm
                .source_contents()
                .map(|cs| cs.map(|c| c.to_owned()))
                .collect(),
        }
    }
}

impl From<RawSourceMap> for sourcemap::SourceMap {
    fn from(rsm: RawSourceMap) -> Self {
        Self::new(
            rsm.file,
            rsm.tokens,
            rsm.names,
            rsm.sources,
            Some(rsm.sources_content),
        )
    }
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
