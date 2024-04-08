use std::collections::HashMap;
use std::sync::Arc;

use cached::proc_macro::cached;
use mako_core::anyhow::{anyhow, Result};
use mako_core::cached::SizedCache;
use mako_core::rayon::prelude::*;
use mako_core::swc_ecma_codegen::text_writer::JsWriter;
use mako_core::swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use mako_core::ternary;

use crate::chunk::Chunk;
use crate::chunk_pot::ast_impl::{render_css_chunk, render_css_chunk_no_cache};
use crate::chunk_pot::util::runtime_code;
use crate::chunk_pot::ChunkPot;
use crate::compiler::Context;
use crate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::module::{Module, ModuleAst};
use crate::sourcemap::build_source_map_to_buf;
use crate::util::base64_encode;

pub(super) fn render_entry_js_chunk(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    hmr_hash: u64,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    let mut files = vec![];
    let mut lines = vec![];

    lines.push(format!(
        "var chunksIdToUrlMap= {};",
        serde_json::to_string(js_map).unwrap()
    ));

    if pot.stylesheet.is_some() {
        mako_core::mako_profile_scope!("CssChunk");
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

    // var cssInstalledChunks = { "chunk_id": 0 }
    let init_install_css_chunk = format!(
        r#"var cssInstalledChunks = {{ "{}" : 0 }};"#,
        pot.chunk_id.clone()
    );

    lines.push(init_install_css_chunk);
    lines.push(format!("var e = \"{}\";", pot.chunk_id));

    let runtime_content = runtime_code(context)?.replace("_%full_hash%_", &hmr_hash.to_string());

    let (chunk_content, chunk_sourcemap) = pot_to_chunk_module_object_string(pot, context)?;
    let mut content: Vec<u8> = format!("var m = {};", chunk_content).into();

    {
        mako_core::mako_profile_scope!("assemble");

        content.splice(0..0, "!(function(){\n".bytes());
        content.extend(lines.join("\n").into_bytes());
        content.extend(runtime_content.into_bytes());
        content.extend("\n})();".as_bytes());
    }

    Ok(ChunkFile {
        raw_hash: hmr_hash,
        content,
        hash: None,
        source_map: Some(chunk_sourcemap),
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
    mako_core::mako_profile_function!(&chunk_pot.js_name);

    let (content_buf, source_map) = pot_to_chunk_module_content(chunk_pot, context)?;

    Ok(ChunkFile {
        raw_hash: chunk_pot.js_hash,
        content: content_buf.into(),
        hash: None,
        source_map: Some(source_map),
        file_name: chunk_pot.js_name.clone(),
        chunk_id: chunk_pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

pub fn pot_to_chunk_module_content(
    pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<(String, Vec<u8>)> {
    let (chunk_content, chunk_sourcemap) = pot_to_chunk_module_object_string(pot, context)?;

    Ok((
        format!(
            r#"(globalThis['{}'] = globalThis['{}'] || []).push([['{}'],
{}]);"#,
            context.config.output.chunk_loading_global,
            context.config.output.chunk_loading_global,
            pot.chunk_id,
            chunk_content
        ),
        chunk_sourcemap,
    ))
}

#[cached(
    result = true,
    key = "String",
    type = "SizedCache<String , (String, Vec<u8>)>",
    create = "{ SizedCache::with_size(20000) }",
    convert = r#"{format!("{}-{}", _raw_hash, module_id_str)}"#
)]
fn emit_module_with_sourcemap(
    fn_expr: &Module,
    context: &Arc<Context>,
    _raw_hash: u64, // used for cache key
    module_id_str: &str,
) -> Result<(String, Vec<u8>)> {
    mako_core::mako_profile_function!(module_id_str);

    match &fn_expr.info.as_ref().unwrap().ast {
        ModuleAst::Script(ast) => {
            mako_core::mako_profile_scope!("ast_to_js_map");
            let mut buf = vec![];
            let mut source_map_buf = Vec::new();
            let cm = context.meta.script.cm.clone();
            let comments = context.meta.script.origin_comments.read().unwrap();
            let swc_comments = comments.get_swc_comments();

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
                    Some(&mut source_map_buf),
                )),
            };

            emitter.emit_module(&ast.ast)?;

            let source_map = build_source_map_to_buf(&source_map_buf, &cm);

            let content = {
                mako_core::mako_profile_scope!("escape");
                String::from_utf8_lossy(&buf)
            };

            Ok((
                format!(
                    r#""{}" : function (module, exports, __mako_require__){{
    {}
  }},"#,
                    module_id_str, content
                ),
                source_map,
            ))
        }
        ModuleAst::Css(_) => Ok((
            format!(
                r#""{}" : function (module, exports, __mako_require__){{
  }},"#,
                module_id_str,
            ),
            Vec::new(),
        )),

        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) not supported", module_id_str)),
    }
}

fn pot_to_chunk_module_object_string(
    pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<(String, Vec<u8>)> {
    mako_core::mako_profile_function!(&pot.chunk_id);

    let sorted_kv = {
        mako_core::mako_profile_scope!("collect_&_sort");

        let mut sorted_kv = pot
            .module_map
            .iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<_>>();

        if context.config.hash {
            sorted_kv.sort_by_key(|(k, _)| *k);
        }

        sorted_kv
    };

    let modules_with_sourcemap = sorted_kv
        .par_iter()
        .map(|(module_id_str, module_and_hash)| {
            emit_module_with_sourcemap(module_and_hash.0, context, module_and_hash.1, module_id_str)
        })
        .collect::<Result<Vec<(String, Vec<u8>)>>>()?;

    let (module_defines, module_sourcemaps) = modules_with_sourcemap.iter().fold(
        (String::new(), Vec::<u8>::new()),
        |(mut module_defines, mut module_sourcemaps), (module_define, module_sourcemap)| {
            module_defines.push('\n');
            module_defines.push_str(module_define);
            module_sourcemaps.extend(module_sourcemap);
            (module_defines, module_sourcemaps)
        },
    );

    Ok((format!(r#"{{ {} }}"#, module_defines), module_sourcemaps))
}
