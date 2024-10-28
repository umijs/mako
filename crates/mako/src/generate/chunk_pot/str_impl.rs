use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cached::proc_macro::cached;
use cached::SizedCache;
use rayon::prelude::*;
use swc_core::base::sourcemap;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::codegen::{Config as JsCodegenConfig, Emitter};

use crate::ast::sourcemap::{build_source_map, RawSourceMap};
use crate::compiler::Context;
use crate::generate::chunk::{Chunk, ChunkType};
use crate::generate::chunk_pot::ast_impl::{render_css_chunk, render_css_chunk_no_cache};
use crate::generate::chunk_pot::util::runtime_code;
use crate::generate::chunk_pot::ChunkPot;
use crate::generate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::module::{generate_module_id, Module, ModuleAst};
use crate::ternary;

pub(super) fn render_entry_js_chunk(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    hmr_hash: u64,
) -> Result<ChunkFile> {
    crate::mako_profile_function!();

    let mut files = vec![];
    let mut lines = vec![];

    lines.push(format!(
        "var chunksIdToUrlMap= {};",
        serde_json::to_string(js_map).unwrap()
    ));

    if pot.stylesheet.is_some() {
        crate::mako_profile_scope!("CssChunk");
        let css_chunk_file = ternary!(
            context.args.watch,
            render_css_chunk,
            render_css_chunk_no_cache
        )(pot, chunk, context)?;

        let mut css_map = css_map.clone();
        css_map.insert(css_chunk_file.chunk_id.clone(), css_chunk_file.disk_name());
        lines.push(format!(
            "var cssChunksIdToUrlMap= {};",
            serde_json::to_string(&css_map).unwrap()
        ));

        files.push(css_chunk_file);
    } else {
        lines.push(format!(
            "var cssChunksIdToUrlMap= {};",
            serde_json::to_string(css_map).unwrap()
        ));
    }

    let chunk_root_module_id = match &chunk.chunk_type {
        ChunkType::Entry(module_id, _, false) | ChunkType::Worker(module_id) => {
            generate_module_id(&module_id.id, context)
        }
        _ => panic!("only entry chunk or worker chunk can be rendered here."),
    };

    // var cssInstalledChunks = { "chunk_id": 0 }
    let init_install_css_chunk = format!(
        r#"var cssInstalledChunks = {{ "{}" : 0 }};"#,
        chunk_root_module_id
    );

    lines.push(init_install_css_chunk);
    lines.push(format!("var e = \"{}\";", chunk_root_module_id));

    let runtime_content = runtime_code(context)?.replace("_%full_hash%_", &hmr_hash.to_string());

    let entry_prefix_code = "!(function(){\n";

    let (chunk_content, chunk_raw_sourcemap) =
        pot_to_chunk_module_object_string(pot, context, entry_prefix_code.lines().count() as u32)?;

    let mut content: Vec<u8> = format!("var m = {};", chunk_content).into();

    {
        content.splice(0..0, entry_prefix_code.bytes());
        content.extend(lines.join("\n").into_bytes());
        content.extend(runtime_content.into_bytes());
        content.extend("\n})();".as_bytes());
    }

    let mut source_map_buf: Vec<u8> = vec![];
    sourcemap::SourceMap::from(chunk_raw_sourcemap).to_writer(&mut source_map_buf)?;

    Ok(ChunkFile {
        raw_hash: hmr_hash,
        content,
        hash: None,
        source_map: Some(source_map_buf),
        file_name: pot.js_name.clone(),
        chunk_id: pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[cached(
    result = true,
    type = "SizedCache<String , ChunkFile>",
    create = "{ SizedCache::with_size(500) }",
    key = "String",
    convert = r#"{format!("{}.{:x}", chunk_pot.chunk_id, chunk_pot.js_hash)}"#
)]
pub(super) fn render_normal_js_chunk(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<ChunkFile> {
    let (content_buf, source_map_buf) = {
        let pot = chunk_pot;

        // to avoid ' or " been included in chunk_loading_global
        let safe_prop = serde_json::to_string(&context.config.output.chunk_loading_global).unwrap();

        let chunk_prefix_code = format!(
            r#"((typeof globalThis !== 'undefined' ? globalThis : self)[{}] = (typeof globalThis !== 'undefined' ? globalThis : self)[{}] || []).push([
        ['{}'],"#,
            safe_prop, safe_prop, pot.chunk_id,
        );

        let (chunk_content, chunk_raw_sourcemap) = pot_to_chunk_module_object_string(
            pot,
            context,
            chunk_prefix_code.lines().count() as u32,
        )?;

        let mut source_map_buf: Vec<u8> = vec![];
        sourcemap::SourceMap::from(chunk_raw_sourcemap).to_writer(&mut source_map_buf)?;

        (
            format!("{}\n{}]);", chunk_prefix_code, chunk_content),
            source_map_buf,
        )
    };

    Ok(ChunkFile {
        raw_hash: chunk_pot.js_hash,
        content: content_buf.into(),
        hash: None,
        source_map: Some(source_map_buf),
        file_name: chunk_pot.js_name.clone(),
        chunk_id: chunk_pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

type EmittedWithMapping = (String, Option<RawSourceMap>);

#[cached(
    result = true,
    key = "String",
    type = "SizedCache<String , EmittedWithMapping>",
    create = "{ SizedCache::with_size(20000) }",
    convert = r#"{format!("{}-{}", _raw_hash, module_id)}"#
)]
fn emit_module_with_mapping(
    module_id: &str,
    module: &Module,
    _raw_hash: u64, // used for cache key
    context: &Arc<Context>,
) -> Result<EmittedWithMapping> {
    match &module.info.as_ref().unwrap().ast {
        ModuleAst::Script(ast) => {
            let cm = context.meta.script.cm.clone();
            let comments = context.meta.script.origin_comments.read().unwrap();
            let swc_comments = comments.get_swc_comments();

            let mut buf = vec![];
            let mut source_mappings = Vec::new();
            let mut emitter = Emitter {
                cfg: JsCodegenConfig::default()
                    .with_minify(false)
                    .with_target(context.config.output.es_version)
                    .with_ascii_only(false)
                    .with_omit_last_semi(true),
                cm: cm.clone(),
                comments: Some(swc_comments),
                wr: Box::new(JsWriter::new(
                    cm.clone(),
                    "\n",
                    &mut buf,
                    Some(&mut source_mappings),
                )),
            };
            emitter.emit_module(&ast.ast)?;

            let source_map = build_source_map(&source_mappings, &cm);

            let content = { String::from_utf8_lossy(&buf) };
            Ok((
                format!(
                    r#""{}": function (module, exports, __mako_require__){{
{}
}},
"#,
                    module_id, content
                ),
                Some(source_map.into()),
            ))
        }
        ModuleAst::Css(_) => Ok((
            format!(
                r#""{}" : function (module, exports, __mako_require__){{
  }},"#,
                module_id,
            ),
            None,
        )),

        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) not supported", module_id)),
    }
}

fn pot_to_chunk_module_object_string(
    pot: &ChunkPot,
    context: &Arc<Context>,
    chunk_prefix_offset: u32,
) -> Result<(String, RawSourceMap)> {
    let sorted_kv = {
        let mut sorted_kv = pot.module_map.iter().collect::<Vec<_>>();

        sorted_kv.sort_by_key(|(k, _)| *k);

        sorted_kv
    };

    let emitted_modules_with_mapping = sorted_kv
        .par_iter()
        .map(|(module_id, module_and_hash)| {
            emit_module_with_mapping(module_id, module_and_hash.0, module_and_hash.1, context)
        })
        .collect::<Result<Vec<(String, Option<RawSourceMap>)>>>()?;

    let (chunk_content, chunk_raw_sourcemap) =
        merge_code_and_sourcemap(emitted_modules_with_mapping, chunk_prefix_offset);

    Ok((format!(r#"{{ {} }}"#, chunk_content), chunk_raw_sourcemap))
}

fn merge_code_and_sourcemap(
    modules_with_sourcemap: Vec<EmittedWithMapping>,
    chunk_prefix_offset: u32,
) -> (String, RawSourceMap) {
    let mut dst_line_offset = 0u32;
    let mut src_id_offset = 0u32;
    let mut name_id_offset = 0u32;
    let (chunk_content, chunk_raw_sourcemap) = modules_with_sourcemap.into_iter().fold(
        (String::new(), RawSourceMap::default()),
        |(mut chunk_content, mut chunk_raw_sourcemap), (module_content, source_mapping)| {
            chunk_content.push_str(&module_content);

            if let Some(mut mappings) = source_mapping {
                chunk_raw_sourcemap
                    .tokens
                    .extend(mappings.tokens.drain(0..).map(|t| sourcemap::RawToken {
                        // 1. in emit_module_with_sourcemap, we have added 1 line code before module output,
                        //    need to add 1
                        // 2. we also have added some prefix code lines in entry chunks or normal
                        //    chunks before chunk output, which it's lines count been stored in PrefixCode,
                        //    need to add its line count
                        // 3. we need to add all code lines count of modules before current
                        dst_line: t.dst_line + 1 + chunk_prefix_offset + dst_line_offset,
                        src_id: t.src_id + src_id_offset,
                        name_id: t.name_id + name_id_offset,
                        ..t
                    }));

                chunk_raw_sourcemap.names.extend(mappings.names.drain(0..));

                chunk_raw_sourcemap
                    .sources
                    .extend(mappings.sources.drain(0..));

                chunk_raw_sourcemap
                    .sources_content
                    .extend(mappings.sources_content.drain(0..));

                name_id_offset = chunk_raw_sourcemap.names.len() as u32;
                src_id_offset = chunk_raw_sourcemap.sources.len() as u32;
                dst_line_offset += module_content.lines().count() as u32;
            }

            (chunk_content, chunk_raw_sourcemap)
        },
    );
    (chunk_content, chunk_raw_sourcemap)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use swc_core::base::sourcemap;
    use swc_core::common::comments::Comments;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::codegen::text_writer::JsWriter;
    use swc_core::ecma::codegen::{Config as JsCodegenConfig, Emitter};
    use swc_core::ecma::transforms::base::hygiene::hygiene_with_config;
    use swc_core::ecma::transforms::base::{hygiene, resolver};
    use swc_core::ecma::visit::VisitMutWith;

    use super::{merge_code_and_sourcemap, EmittedWithMapping};
    use crate::ast::js_ast::JsAst;
    use crate::ast::sourcemap::build_source_map;
    use crate::compiler::{Args, Context};
    use crate::config::{Config, Mode};
    use crate::generate::minify::minify_js;

    #[test]
    fn test_pot_to_chunk_module_object_string() {
        let context = Arc::new(Context {
            config: Config {
                mode: Mode::Development,
                minify: true,
                ..Default::default()
            },
            args: Args { watch: true },
            ..Default::default()
        });

        GLOBALS.set(&context.meta.script.globals, || {
            let emitted_add = build_file(
                "add.js",
                r#"function add(a,b) {
  const a_1 = parseInt(a);
  const b_1 = parseInt(b);
  return a_1 + b_1;
}"#,
                &context,
            )
            .unwrap();

            let emitted_sub = build_file(
                "sub.js",
                r#"function sub(a,b) {
  const a_1 = parseInt(a);
  const b_1 = parseInt(b);
  return a_1 - b_1;
}"#,
                &context,
            )
            .unwrap();

            let emitted_add_code = emitted_add.0.clone();
            let emitted_add_sourcemap: sourcemap::SourceMap =
                emitted_add.1.as_ref().unwrap().clone().into();
            let emitted_sub_sourcemap: sourcemap::SourceMap =
                emitted_sub.1.as_ref().unwrap().clone().into();

            let chunk_prefix_offset = 1u32;

            let merged_code_and_sourcemap =
                merge_code_and_sourcemap(vec![emitted_add, emitted_sub], chunk_prefix_offset);

            let merged_sourcemap: sourcemap::SourceMap = merged_code_and_sourcemap.1.into();

            // in fn emit_module_with_sourcemap, we add 1 line prefix code before module output
            let emit_module_with_sourcemap_gap = 1u32;

            assert_eq!(
                emitted_add_sourcemap
                    .tokens()
                    .map(|t| t.get_dst_line()
                        + chunk_prefix_offset
                        + emit_module_with_sourcemap_gap)
                    .collect::<Vec<u32>>(),
                merged_sourcemap
                    .tokens()
                    .filter(|t| t.get_source().unwrap() == "add.js")
                    .map(|t| t.get_dst_line())
                    .collect::<Vec<u32>>()
            );

            assert_eq!(
                emitted_sub_sourcemap
                    .tokens()
                    .map(|t| t.get_dst_line()
                        + chunk_prefix_offset
                        + emit_module_with_sourcemap_gap
                        + emitted_add_code.lines().count() as u32)
                    .collect::<Vec<u32>>(),
                merged_sourcemap
                    .tokens()
                    .filter(|t| t.get_source().unwrap() == "sub.js")
                    .map(|t| t.get_dst_line())
                    .collect::<Vec<u32>>()
            );
        });
    }

    fn build_file(file: &str, code: &str, context: &Arc<Context>) -> Result<EmittedWithMapping> {
        let mut ast = JsAst::build(file, code, context.clone()).unwrap();

        let top = ast.top_level_mark;
        ast.ast
            .visit_mut_with(&mut resolver(ast.unresolved_mark, top, false));
        ast.ast
            .visit_mut_with(&mut hygiene_with_config(hygiene::Config {
                top_level_mark: top,
                ..Default::default()
            }));

        minify_js(&mut ast, context).unwrap();

        let mut buf = vec![];
        let mut source_map_buf = Vec::new();
        let cm = &context.meta.script.cm;
        let comments = context.meta.script.origin_comments.read().unwrap();
        let swc_comments = comments.get_swc_comments();
        {
            let with_minify =
                context.config.minify && matches!(context.config.mode, Mode::Production);
            let mut emitter = Emitter {
                cfg: JsCodegenConfig::default()
                    .with_minify(with_minify)
                    .with_target(context.config.output.es_version)
                    .with_ascii_only(with_minify)
                    .with_omit_last_semi(true),
                cm: cm.clone(),
                comments: (!with_minify).then_some(swc_comments as &dyn Comments),
                wr: Box::new(JsWriter::new(
                    cm.clone(),
                    "\n",
                    &mut buf,
                    Some(&mut source_map_buf),
                )),
            };
            emitter.emit_module(&ast.ast)?;
        }

        let code = String::from_utf8(buf)?;
        let source_map = build_source_map(&source_map_buf, cm);
        Ok((code, Some(source_map.into())))
    }
}
