use mako_core::swc_common::source_map::SourceMapGenConfig;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{BytePos, FileName, LineCol, SourceMap};

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

pub fn build_source_map(mappings: &[(BytePos, LineCol)], cm: &Lrc<SourceMap>) -> Vec<u8> {
    let config = SwcSourceMapGenConfig;

    let mut src_buf = vec![];

    cm.build_source_map_with_config(mappings, None, config)
        .to_writer(&mut src_buf)
        .unwrap();

    src_buf
}
