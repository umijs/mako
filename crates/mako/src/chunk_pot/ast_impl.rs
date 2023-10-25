use std::collections::HashMap;
use std::sync::Arc;

use cached::proc_macro::cached;
use mako_core::anyhow::Result;
use mako_core::cached::SizedCache;
use mako_core::swc_common::{Mark, DUMMY_SP, GLOBALS};
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use mako_core::swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use mako_core::swc_ecma_ast::{
    KeyValueProp, Lit, Number, ObjectLit, Prop, PropOrSpread, Stmt, VarDeclKind,
};
use mako_core::swc_ecma_utils::{quote_ident, quote_str, ExprFactory};

use crate::ast::{build_js_ast, Ast};
use crate::chunk::{Chunk, ChunkType};
use crate::chunk_pot::util::{
    hash_hashmap, pot_to_chunk_module, pot_to_module_object, runtime_code,
};
use crate::chunk_pot::{get_css_chunk_filename, util, ChunkPot};
use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::load::file_content_hash;
use crate::minify::{minify_css, minify_js};
use crate::sourcemap::build_source_map;
use crate::transform_in_generate::transform_css_generate;

#[cached(
    result = true,
    key = "u64",
    convert = "{chunk_pot.stylesheet.as_ref().unwrap().raw_hash}"
)]
pub(crate) fn render_css_chunk(chunk_pot: &ChunkPot, context: &Arc<Context>) -> Result<ChunkFile> {
    mako_core::mako_profile_function!(&chunk_pot.js_name);
    let mut css_code = String::new();
    let mut source_map = Vec::new();
    let css_writer = BasicCssWriter::new(
        &mut css_code,
        Some(&mut source_map),
        BasicCssWriterConfig::default(),
    );

    let ast = &mut chunk_pot.stylesheet.as_ref().unwrap();

    let mut stylesheet = Stylesheet {
        span: DUMMY_SP,
        rules: ast
            .stylesheets
            .iter()
            .flat_map(|stylesheet| stylesheet.rules.clone())
            .collect(),
    };

    {
        mako_core::mako_profile_scope!("transform_css_generate");
        transform_css_generate(&mut stylesheet, context);
    }

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_css(&mut stylesheet, context)?;
    }

    let mut gen = CodeGenerator::new(
        css_writer,
        CodegenConfig {
            minify: context.config.minify && matches!(context.config.mode, Mode::Production),
        },
    );
    gen.emit(&stylesheet)?;

    let cm = &context.meta.css.cm;
    let source_map = match context.config.devtool {
        DevtoolConfig::None => None,
        _ => Some(build_source_map(&source_map, cm)),
    };

    let css_hash = if context.config.hash {
        Some(file_content_hash(&css_code))
    } else {
        None
    };

    Ok(ChunkFile {
        raw_hash: ast.raw_hash,
        content: css_code.into(),
        hash: css_hash,
        source_map,
        file_name: get_css_chunk_filename(&chunk_pot.js_name),
        chunk_id: chunk_pot.chunk_id.clone(),
        file_type: ChunkFileType::Css,
    })
}

#[cached(
    result = true,
    type = "SizedCache<u64 , ChunkFile>",
    create = "{ SizedCache::with_size(500) }",
    key = "u64",
    convert = "{chunk_pot.js_hash}"
)]
pub(crate) fn render_normal_js_chunk(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    let module = pot_to_chunk_module(chunk_pot)?;

    let mut ast = GLOBALS.set(&context.meta.script.globals, || Ast {
        ast: module,
        unresolved_mark: Mark::new(),
        top_level_mark: Mark::new(),
    });

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_js(&mut ast, context)?;
    }

    let (buf, source_map) = util::render_module_js(&ast.ast, context)?;

    let hash = if context.config.hash {
        Some(file_content_hash(&buf))
    } else {
        None
    };

    Ok(ChunkFile {
        raw_hash: chunk_pot.js_hash,
        content: buf,
        hash,
        source_map,
        file_name: chunk_pot.js_name.clone(),
        chunk_id: chunk_pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{:x}",pot.js_hash
         .wrapping_add(hash_hashmap(js_map))
         .wrapping_add(hash_hashmap(css_map))
         .wrapping_add(full_hash)) }"#
)]
pub(crate) fn render_entry_js_chunk(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    full_hash: u64,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    let (buf, source_map_buf, hash) =
        render_entry_chunk_js_without_full_hash(pot, js_map, css_map, chunk, context)?;

    let content = {
        mako_core::mako_profile_scope!("full_hash_replace");

        String::from_utf8(buf)?
            .replace("_%full_hash%_", &full_hash.to_string())
            .into_bytes()
    };

    Ok(ChunkFile {
        raw_hash: pot.js_hash,
        content,
        hash,
        source_map: source_map_buf,
        file_name: pot.js_name.clone(),
        chunk_id: pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{format!("{}",pot.js_hash)}"#
)]
fn render_entry_chunk_js_without_full_hash(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
) -> Result<(Vec<u8>, Option<Vec<u8>>, Option<String>)> {
    mako_core::mako_profile_function!(&pot.chunk_id);

    let mut stmts = vec![];

    let (js_map_stmt, css_map_stmt) = chunk_map_decls(js_map, css_map);

    stmts.push(js_map_stmt);
    stmts.push(css_map_stmt);

    if let ChunkType::Entry(module_id, _) = &chunk.chunk_type {
        let main_id_decl: Stmt = quote_str!(module_id.generate(context))
            .into_var_decl(VarDeclKind::Var, quote_ident!("e").into()) // e brief for entry_module_id
            .into();

        stmts.push(main_id_decl);
    }

    // var cssInstalledChunks = { "chunk_id": 0 }
    let init_install_css_chunk: Stmt = {
        ObjectLit {
            span: DUMMY_SP,
            props: vec![Prop::KeyValue(KeyValueProp {
                key: quote_str!(pot.chunk_id.clone()).into(),
                value: Lit::Num(Number {
                    span: DUMMY_SP,
                    value: 0f64,
                    raw: None,
                })
                .into(),
            })
            .into()],
        }
        .into_var_decl(VarDeclKind::Var, quote_ident!("cssInstalledChunks").into())
        .into()
    };

    stmts.push(init_install_css_chunk);

    let mut ast = {
        mako_core::mako_profile_scope!("parse_runtime_entry");

        let runtime_content = runtime_code(context)?;

        build_js_ast(
            "_mako_internal/runtime_entry.js",
            runtime_content.as_str(),
            context,
        )
        .unwrap()
    };

    let modules_lit: Stmt = {
        mako_core::mako_profile_scope!("to_module_object");

        pot_to_module_object(pot)?
            .into_var_decl(VarDeclKind::Var, quote_ident!("m").into())
            .into()
    };

    {
        mako_core::mako_profile_scope!("entryInsert");

        ast.ast.body.insert(0, modules_lit.into());

        ast.ast
            .body
            .splice(0..0, stmts.into_iter().map(|s| s.into()));
    }

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_js(&mut ast, context)?;
    }

    let (buf, source_map_buf) = util::render_module_js(&ast.ast, context)?;

    let hash = if context.config.hash {
        mako_core::mako_profile_scope!("entryHash");
        Some(file_content_hash(&buf))
    } else {
        None
    };

    Ok((buf, source_map_buf, hash))
}

fn chunk_map_decls(
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
) -> (Stmt, Stmt) {
    let js_chunk_map_dcl_stmt: Stmt = to_object_lit(js_map)
        .into_var_decl(VarDeclKind::Var, quote_ident!("chunksIdToUrlMap").into())
        .into();

    let css_chunk_map_dcl_stmt: Stmt = to_object_lit(css_map)
        .into_var_decl(VarDeclKind::Var, quote_ident!("cssChunksIdToUrlMap").into())
        .into();

    (js_chunk_map_dcl_stmt, css_chunk_map_dcl_stmt)
}

fn to_object_lit(value: &HashMap<String, String>) -> ObjectLit {
    let props = value
        .iter()
        .map(|(k, v)| {
            Prop::KeyValue(KeyValueProp {
                key: quote_str!(k.clone()).into(),
                value: quote_str!(v.clone()).into(),
            })
            .into()
        })
        .collect::<Vec<PropOrSpread>>();

    ObjectLit {
        span: DUMMY_SP,
        props,
    }
}
