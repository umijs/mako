use std::collections::HashMap;
use std::path::PathBuf;

use pathdiff::diff_paths;
use swc_core::base::sourcemap as swc_sourcemap;
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
) -> swc_sourcemap::SourceMap {
    let config = SwcSourceMapGenConfig;

    cm.build_source_map_with_config(mappings, None, config)
}

// Add this type because the sourcemap::SourceMap type can't be cached,
// there is a RefCell type field in it
#[derive(Clone, Default, Debug)]
pub struct RawSourceMap {
    pub file: Option<String>,
    pub tokens: Vec<swc_sourcemap::RawToken>,
    pub names: Vec<String>,
    pub sources: Vec<String>,
    pub sources_content: Vec<Option<String>>,
}

impl From<swc_sourcemap::SourceMap> for RawSourceMap {
    fn from(sm: swc_sourcemap::SourceMap) -> Self {
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

impl From<RawSourceMap> for swc_sourcemap::SourceMap {
    fn from(rsm: RawSourceMap) -> Self {
        Self::new(
            rsm.file.map(|f| f.into_boxed_str().into()),
            rsm.tokens,
            rsm.names
                .into_iter()
                .map(|n| n.into_boxed_str().into())
                .collect(),
            rsm.sources
                .into_iter()
                .map(|n| n.into_boxed_str().into())
                .collect(),
            Some(
                rsm.sources_content
                    .into_iter()
                    .map(|op_string| op_string.map(|s| s.into_boxed_str().into()))
                    .collect(),
            ),
        )
    }
}

// This is based on https://github.com/jiesia/merge-source-map/blob/main/src/lib.rs#L95,
// just refactor it with a hash map to determinate which source should be searched accurately
pub fn merge_source_map(
    target_source_map: swc_sourcemap::SourceMap,
    chain_map: HashMap<String, Vec<swc_sourcemap::SourceMap>>,
    root: &PathBuf,
) -> swc_sourcemap::SourceMap {
    let mut builder = swc_sourcemap::SourceMapBuilder::new(None);
    target_source_map.tokens().for_each(|token| {
        if let Some(source) = token.get_source() {
            let mut final_token = token;
            let mut searched_in_chain = true;

            if let Some(source_map_chain) = chain_map.get(source)
                && !source_map_chain.is_empty()
            {
                for map in source_map_chain.iter().rev() {
                    if let Some(map_token) =
                        map.lookup_token(token.get_src_line(), token.get_src_col())
                    {
                        final_token = map_token;
                    } else {
                        searched_in_chain = false;
                        break;
                    }
                }
            }

            // This maybe impossible ?
            if !searched_in_chain {
                return;
            }

            // replace source
            let replaced_source = final_token.get_source().map(|src| {
                diff_paths(src, root)
                    .unwrap_or(src.into())
                    .to_string_lossy()
                    .to_string()
            });

            // add mapping
            let added_token = builder.add(
                token.get_dst_line(),
                token.get_dst_col(),
                final_token.get_src_line(),
                final_token.get_src_col(),
                replaced_source.as_deref(),
                final_token.get_name(),
                false,
            );

            // add source content
            if !builder.has_source_contents(added_token.src_id) {
                let source_content = final_token.get_source_view().map(|view| view.source());

                builder.set_source_contents(added_token.src_id, source_content);
            }
        }
    });

    builder.into_sourcemap()
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::str::FromStr;

    use crate::ast::sourcemap::{merge_source_map, swc_sourcemap};

    #[test]
    fn test_merge_empty_chain() {
        let sourcemap2 = r#"{
            "version": 3,
            "file": "minify.js",
            "sourceRoot": "",
            "sources": [
              "index.ts"
            ],
            "names": [
              "sayHello",
              "name",
              "console",
              "log",
              "concat"
            ],
            "mappings": "AAAA,SAASA,SAASC,CAAI,EAClBC,QAAQC,GAAG,CAAC,UAAUC,MAAM,CAACH,GACjC",
            "sourcesContent": [
              "function sayHello(name) {\n    console.log(\"Hello, \".concat(name));\n}\n"
            ]
        }"#;

        let merged_source_map = merge_source_map(
            swc_sourcemap::SourceMap::from_reader(sourcemap2.as_bytes()).unwrap(),
            HashMap::<String, Vec<swc_sourcemap::SourceMap>>::new(),
            &PathBuf::from_str("./").unwrap(),
        );

        let mut buf = vec![];

        merged_source_map.to_writer(&mut buf).unwrap();

        let merged = String::from_utf8(buf).unwrap();

        assert!(merged.eq(r#"{"version":3,"sources":["index.ts"],"sourcesContent":["function sayHello(name) {\n    console.log(\"Hello, \".concat(name));\n}\n"],"names":["sayHello","name","console","log","concat"],"mappings":"AAAA,SAASA,SAASC,CAAI,EAClBC,QAAQC,GAAG,CAAC,UAAUC,MAAM,CAACH,GACjC"}"#))
    }

    #[test]
    fn test_merge() {
        let sourcemap1 = r#"{
            "version": 3,
            "file": "index.js",
            "sourceRoot": "",
            "sources": [
              "index.ts"
            ],
            "names": [],
            "mappings": "AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC",
            "sourcesContent": [
              "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
            ]
        }"#;
        let sourcemap2 = r#"{
            "version": 3,
            "file": "minify.js",
            "sourceRoot": "",
            "sources": [
              "index.ts"
            ],
            "names": [
              "sayHello",
              "name",
              "console",
              "log",
              "concat"
            ],
            "mappings": "AAAA,SAASA,SAASC,CAAI,EAClBC,QAAQC,GAAG,CAAC,UAAUC,MAAM,CAACH,GACjC",
            "sourcesContent": [
              "function sayHello(name) {\n    console.log(\"Hello, \".concat(name));\n}\n"
            ]
        }"#;

        let merged_source_map = merge_source_map(
            swc_sourcemap::SourceMap::from_reader(sourcemap2.as_bytes()).unwrap(),
            HashMap::<String, Vec<swc_sourcemap::SourceMap>>::from([(
                "index.ts".to_string(),
                vec![swc_sourcemap::SourceMap::from_reader(sourcemap1.as_bytes()).unwrap()],
            )]),
            &PathBuf::from_str("./").unwrap(),
        );

        let mut buf = vec![];

        merged_source_map.to_writer(&mut buf).unwrap();

        let merged = String::from_utf8(buf).unwrap();

        assert!(merged.eq(r#"{"version":3,"sources":["index.ts"],"sourcesContent":["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"],"names":[],"mappings":"AAAA,SAAS,SAAS,CAAY,EAC5B,QAAQ,GAAG,CAAC,UAAA,MAAA,CAAU,GACxB"}"#));
    }
}
