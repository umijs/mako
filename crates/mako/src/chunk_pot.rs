use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::Arc;
use std::vec;

use cached::proc_macro::cached;
#[allow(unused_imports)]
use cached::proc_macro::once;
use mako_core::anyhow::{anyhow, Result};
use mako_core::cached::SizedCache;
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::swc_common::{Mark, DUMMY_SP, GLOBALS};
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use mako_core::swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use mako_core::swc_ecma_ast::{
    ArrayLit, BlockStmt, Expr, ExprOrSpread, FnExpr, Function, KeyValueProp, Lit,
    Module as SwcModule, Number, ObjectLit, Prop, PropOrSpread, Stmt, VarDeclKind,
};
use mako_core::swc_ecma_codegen::text_writer::JsWriter;
use mako_core::swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use mako_core::swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use mako_core::tracing::debug;
use mako_core::twox_hash::XxHash64;

use crate::ast::{base64_encode, build_js_ast, Ast};
use crate::chunk::{Chunk, ChunkType};
use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::load::file_content_hash;
use crate::minify::{minify_css, minify_js};
use crate::module::{Module, ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;
use crate::sourcemap::build_source_map;
use crate::transform_in_generate::transform_css_generate;

pub struct ChunkPot<'a> {
    pub chunk_id: String,
    pub js_name: String,
    pub module_map: HashMap<String, (&'a Module, u64)>,
    pub js_hash: u64,
    stylesheet: Option<CssModules<'a>>,
}

#[cached(
    result = true,
    key = "u64",
    convert = "{chunk_pot.stylesheet.as_ref().unwrap().raw_hash}"
)]
fn render_chunk_css(chunk_pot: &ChunkPot, context: &Arc<Context>) -> Result<ChunkFile> {
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
    let source_map = build_source_map(&source_map, cm);

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
fn render_normal_chunk_js_with_cache(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    let module = chunk_pot.to_chunk_module()?;

    let mut ast = GLOBALS.set(&context.meta.script.globals, || Ast {
        ast: module,
        unresolved_mark: Mark::new(),
        top_level_mark: Mark::new(),
    });

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_js(&mut ast, context)?;
    }

    let (buf, source_map) = render_module_js(&ast.ast, context)?;

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

impl<'cp> ChunkPot<'cp> {
    pub fn from<'a: 'cp>(
        chunk: &'a Chunk,
        mg: &'a ModuleGraph,
        context: &'cp Arc<Context>,
    ) -> Result<Self> {
        let (js_modules, stylesheet) = ChunkPot::split_modules(chunk.get_modules(), mg, context)?;

        Ok(ChunkPot {
            js_name: chunk.filename(),
            chunk_id: chunk.id.generate(context),
            module_map: js_modules.module_map,
            js_hash: js_modules.raw_hash,
            stylesheet,
        })
    }

    pub fn to_normal_chunk_files(&self, context: &Arc<Context>) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!(&self.chunk_id);

        let mut files = vec![];

        let js_chunk_file = render_normal_chunk_js_with_cache(self, context)?;

        files.push(js_chunk_file);

        if self.stylesheet.is_some() {
            let css_chunk_file = render_chunk_css(self, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    pub fn to_entry_chunk_files(
        &self,
        context: &Arc<Context>,
        js_map: &HashMap<String, String>,
        css_map: &HashMap<String, String>,
        chunk: &Chunk,
        full_hash: u64,
    ) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();

        let mut files = vec![];

        if self.stylesheet.is_some() {
            let css_chunk_file = render_chunk_css(self, context)?;

            let mut css_map = css_map.clone();
            css_map.insert(css_chunk_file.chunk_id.clone(), css_chunk_file.disk_name());

            files.push(css_chunk_file);
            files.push(render_entry_chunk_js(
                self, js_map, &css_map, chunk, context, full_hash,
            )?);
        } else {
            mako_core::mako_profile_scope!("EntryDevJsChunk", &self.chunk_id);

            files.push(render_entry_chunk_js(
                self, js_map, css_map, chunk, context, full_hash,
            )?);
        }

        Ok(files)
    }

    fn split_modules<'a>(
        module_ids: &'a IndexSet<ModuleId>,
        module_graph: &'a ModuleGraph,
        context: &'a Arc<Context>,
    ) -> Result<(JsModules<'a>, Option<CssModules<'a>>)> {
        mako_core::mako_profile_function!(module_ids.len().to_string());
        let mut module_map: HashMap<String, (&Module, u64)> = Default::default();
        let mut merged_css_modules: Vec<(String, &Stylesheet)> = vec![];

        let mut module_raw_hash_map: HashMap<String, u64> = Default::default();
        let mut css_raw_hashes = vec![];

        let module_ids: Vec<_> = module_ids.iter().collect();

        for module_id in module_ids {
            let module = module_graph.get_module(module_id).unwrap();
            let module_info = module.info.as_ref().unwrap();
            let ast = &module_info.ast;

            if let ModuleAst::Script(_) = ast {
                module_raw_hash_map.insert(module.id.id.clone(), module_info.raw_hash);
                module_map.insert(module.id.generate(context), (module, module_info.raw_hash));
            }

            if let ModuleAst::Css(ast) = ast {
                // only apply the last css module if chunk depend on it multiple times
                // make sure the rules order is correct
                if let Some(index) = merged_css_modules
                    .iter()
                    .position(|(id, _)| id.eq(&module.id.id))
                {
                    merged_css_modules.remove(index);
                }
                merged_css_modules.push((module.id.id.clone(), ast));
                css_raw_hashes.push(module_info.raw_hash);
            }
        }

        let raw_hash = hash_map_ordered_by_key(&module_raw_hash_map);

        if !merged_css_modules.is_empty() {
            mako_core::mako_profile_scope!("iter_chunk_css_modules");

            let mut stylesheets = vec![];

            for (_, ast) in merged_css_modules {
                stylesheets.push(ast);
            }

            let css_raw_hash = hash_vec(&css_raw_hashes);

            Ok((
                JsModules {
                    module_map,
                    raw_hash,
                },
                Some(CssModules {
                    stylesheets,
                    raw_hash: css_raw_hash,
                }),
            ))
        } else {
            Ok((
                JsModules {
                    module_map,
                    raw_hash,
                },
                None,
            ))
        }
    }

    pub fn to_module_object(&self) -> Result<ObjectLit> {
        mako_core::mako_profile_function!();

        let mut sorted_kv = self
            .module_map
            .iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<_>>();
        sorted_kv.sort_by_key(|(k, _)| *k);

        let mut props = Vec::new();

        for (module_id_str, module) in sorted_kv {
            let fn_expr = to_module_fn_expr(module.0)?;

            let pv: PropOrSpread = Prop::KeyValue(KeyValueProp {
                key: quote_str!(module_id_str.clone()).into(),
                value: fn_expr.into(),
            })
            .into();

            props.push(pv);
        }

        Ok(ObjectLit {
            span: DUMMY_SP,
            props,
        })
    }

    pub fn to_chunk_module(&self) -> Result<SwcModule> {
        mako_core::mako_profile_function!();
        // key: module id
        // value: module FnExpr
        let mut sorted_kv = self
            .module_map
            .iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<_>>();

        sorted_kv.sort_by_key(|(k, _)| *k);

        let module_object = self.to_module_object()?;

        // globalThis.jsonpCallback([["module_id"], { module object }])
        let jsonp_callback_stmt = <Expr as ExprFactory>::as_call(
            *member_expr!(DUMMY_SP, globalThis.jsonpCallback),
            DUMMY_SP,
            // [[ "module id"], { module object }]
            vec![to_array_lit(vec![
                to_array_lit(vec![quote_str!(self.chunk_id.clone()).as_arg()]).as_arg(),
                module_object.as_arg(),
            ])
            .as_arg()],
        )
        .into_stmt();

        Ok(SwcModule {
            body: vec![jsonp_callback_stmt.into()],
            shebang: None,
            span: DUMMY_SP,
        })
    }
}

#[cached(
    result = true,
    key = "u64",
    type = "SizedCache<u64, FnExpr>",
    create = "{ SizedCache::with_size(20000) }",
    convert = r#"{module.info.as_ref().unwrap().raw_hash}"#
)]
pub fn to_module_fn_expr(module: &Module) -> Result<FnExpr> {
    mako_core::mako_profile_function!(&module.id.id);

    match &module.info.as_ref().unwrap().ast {
        ModuleAst::Script(script) => {
            let mut stmts = Vec::new();

            for n in script.ast.body.iter() {
                match n.as_stmt() {
                    None => return Err(anyhow!("Error: {:?} not a stmt in ", module.id.id)),
                    Some(stmt) => {
                        stmts.push(stmt.clone());
                    }
                }
            }

            let func = Function {
                span: DUMMY_SP,
                params: vec![
                    quote_ident!("module").into(),
                    quote_ident!("exports").into(),
                    quote_ident!("require").into(),
                ],
                decorators: vec![],
                body: Some(BlockStmt {
                    span: DUMMY_SP,
                    stmts,
                }),
                is_generator: false,
                is_async: false,
                type_params: None,
                return_type: None,
            };
            Ok(FnExpr {
                ident: None,
                function: func.into(),
            })
        }
        //TODO:  css module will be removed in the future
        ModuleAst::Css(_) => Ok(empty_module_fn_expr()),
        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) cannot concert", module.id.id)),
    }
}

fn empty_module_fn_expr() -> FnExpr {
    let func = Function {
        span: DUMMY_SP,
        params: vec![
            quote_ident!("module").into(),
            quote_ident!("exports").into(),
            quote_ident!("require").into(),
        ],
        decorators: vec![],
        body: Some(BlockStmt {
            span: DUMMY_SP,
            stmts: vec![],
        }),
        is_generator: false,
        is_async: false,
        type_params: None,
        return_type: None,
    };
    FnExpr {
        ident: None,
        function: func.into(),
    }
}

#[cached(
    result = true,
    key = "String",
    type = "SizedCache<String , String>",
    create = "{ SizedCache::with_size(20000) }",
    convert = r#"{format!("{}-{}", _raw_hash, module_id_str)}"#
)]
fn to_module_line(
    fn_expr: &Module,
    context: &Arc<Context>,
    _raw_hash: u64, // used for cache key
    module_id_str: &str,
) -> Result<String> {
    mako_core::mako_profile_function!(module_id_str);

    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
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

    match &fn_expr.info.as_ref().unwrap().ast {
        ModuleAst::Script(ast) => {
            mako_core::mako_profile_scope!("ast_to_js_map");

            emitter.emit_module(&ast.ast)?;

            let source_map = build_source_map(&source_map_buf, &cm);

            let content = {
                mako_core::mako_profile_scope!("escape");
                String::from_utf8_lossy(&buf)
            };
            // let source_map_file = format!("{}.map", file_content_hash(module_id_str));

            let content = vec![
                content,
                format!(
                    "//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                    base64_encode(source_map)
                )
                .into(),
                // format!(
                //     "//# sourceMappingURL={}{}",
                //     context.config.public_path, source_map_file
                // )
                // .into(),
            ]
            .join("");

            // context.write_static_content(&source_map_file, source_map_content)?;

            let escaped = serde_json::to_string(&content)?;

            Ok(format!(
                r#""{}" : function (module, exports, require){{
    eval({})
  }},"#,
                module_id_str, escaped
            ))
        }
        ModuleAst::Css(_) => Ok(format!(
            r#""{}" : function (module, exports, require){{
  }},"#,
            module_id_str,
        )),

        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) not supported", module_id_str)),
    }
}

struct JsModules<'a> {
    pub module_map: HashMap<String, (&'a Module, u64)>,
    raw_hash: u64,
}

struct CssModules<'a> {
    stylesheets: Vec<&'a Stylesheet>,
    raw_hash: u64,
}

fn hash_map_ordered_by_key(map: &HashMap<String, u64>) -> u64 {
    let mut hash: XxHash64 = Default::default();

    let mut sorted = map.iter().map(|(k, v)| (k, v)).collect::<Vec<_>>();

    sorted.sort_by_key(|(k, _)| *k);

    sorted.iter().for_each(|(_, &v)| {
        hash.write_u64(v);
    });

    hash.finish()
}

fn hash_vec(v: &[u64]) -> u64 {
    let mut hash: XxHash64 = Default::default();

    v.iter().for_each(|v| {
        hash.write_u64(*v);
    });

    hash.finish()
}

fn to_array_lit(elems: Vec<ExprOrSpread>) -> ArrayLit {
    ArrayLit {
        span: DUMMY_SP,
        elems: elems.into_iter().map(Some).collect::<Vec<_>>(),
    }
}

fn get_css_chunk_filename(js_chunk_filename: &str) -> String {
    format!(
        "{}.css",
        js_chunk_filename.strip_suffix(".js").unwrap_or("")
    )
}

fn render_module_js(ast: &SwcModule, context: &Arc<Context>) -> Result<(Vec<u8>, Vec<u8>)> {
    mako_core::mako_profile_function!();

    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();

    let mut emitter = Emitter {
        cfg: JsCodegenConfig::default()
            .with_minify(context.config.minify && matches!(context.config.mode, Mode::Production))
            .with_target(context.config.output.es_version)
            .with_ascii_only(true)
            .with_omit_last_semi(true),
        cm: cm.clone(),
        comments: Some(swc_comments),
        wr: Box::new(JsWriter::new(cm, "\n", &mut buf, Some(&mut source_map_buf))),
    };
    emitter.emit_module(ast)?;

    let cm = &context.meta.script.cm;
    let source_map = match context.config.devtool {
        DevtoolConfig::None => {
            vec![]
        }
        _ => build_source_map(&source_map_buf, cm),
    };

    Ok((buf, source_map))
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{:x}",pot.js_hash + hash_hashmap(js_map)+hash_hashmap(css_map)+ full_hash) }"#
)]
fn render_entry_chunk_js(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    full_hash: u64,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    {
        mako_core::mako_profile_scope!("key calculation");
        debug!(
            "render_entry_chunk_js-cache-key {:x}.{:x}.{:x}.{:x}",
            pot.js_hash,
            hash_hashmap(js_map),
            hash_hashmap(css_map),
            full_hash
        );
    }

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
) -> Result<(Vec<u8>, Vec<u8>, Option<String>)> {
    mako_core::mako_profile_function!();

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

        let runtime_content = runtime_base_code_with_cache(context)?;

        build_js_ast(
            "_mako_internal/runtime_entry.js",
            runtime_content.as_str(),
            context,
        )
        .unwrap()
    };

    let modules_lit: Stmt = {
        mako_core::mako_profile_scope!("to_module_object");

        pot.to_module_object()?
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

    let (buf, source_map_buf) = render_module_js(&ast.ast, context)?;

    let hash = if context.config.hash {
        mako_core::mako_profile_scope!("entryHash");
        Some(file_content_hash(&buf))
    } else {
        None
    };

    Ok((buf, source_map_buf, hash))
}

#[cfg(test)]
fn runtime_base_code_with_cache(context: &Arc<Context>) -> Result<String> {
    runtime_code(context)
}

#[cfg(not(test))]
#[once(result = true)]
fn runtime_base_code_with_cache(context: &Arc<Context>) -> Result<String> {
    runtime_code(context)
}

fn runtime_code(context: &Arc<Context>) -> Result<String> {
    let runtime_entry_content_str = include_str!("runtime/runtime_entry.js");
    let mut content = runtime_entry_content_str.replace(
        "// __inject_runtime_code__",
        &context.plugin_driver.runtime_plugins_code(context)?,
    );
    if context.config.umd != "none" {
        let umd_runtime = include_str!("runtime/runtime_umd.js");
        let umd_runtime = umd_runtime.replace("_%umd_name%_", &context.config.umd);
        content.push_str(&umd_runtime);
    }
    Ok(content)
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

fn hash_hashmap(map: &HashMap<String, String>) -> u64 {
    let mut sorted_kv = map.iter().map(|(k, v)| (k, v)).collect::<Vec<_>>();
    sorted_kv.sort_by_key(|(k, _)| *k);

    let mut hasher: XxHash64 = Default::default();
    for c in sorted_kv {
        hasher.write(c.0.as_bytes());
        hasher.write(c.1.as_bytes());
    }
    hasher.finish()
}
