use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::Arc;

use anyhow::Result;
use cached::instant::Instant;
use cached::proc_macro::{cached, once};
use cached::SizedCache;
use indexmap::IndexSet;
use rayon::prelude::*;
use swc_common::{Mark, DUMMY_SP, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use swc_ecma_ast::{
    ArrayLit, Expr, ExprOrSpread, KeyValueProp, Lit, Module as SwcModule, Number, ObjectLit, Prop,
    PropOrSpread, Stmt, VarDeclKind,
};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use twox_hash::XxHash64;

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
    pub stylesheet: Option<(Stylesheet, u64)>,
}

#[cached(
    result = true,
    key = "u64",
    convert = "{chunk_pot.stylesheet.as_ref().unwrap().1}"
)]
fn render_chunk_css(chunk_pot: &mut ChunkPot, context: &Arc<Context>) -> Result<ChunkFile> {
    let mut css_code = String::new();
    let mut source_map = Vec::new();
    let css_writer = BasicCssWriter::new(
        &mut css_code,
        Some(&mut source_map),
        BasicCssWriterConfig::default(),
    );

    let ast = &mut chunk_pot.stylesheet.as_mut().unwrap().0;

    {
        mako_core::mako_profile_scope!("transform_css_generate");
        transform_css_generate(ast, context);
    }

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_css(ast, context)?;
    }

    let mut gen = CodeGenerator::new(
        css_writer,
        CodegenConfig {
            minify: context.config.minify && matches!(context.config.mode, Mode::Production),
        },
    );
    gen.emit(ast)?;

    let cm = &context.meta.css.cm;
    let source_map = build_source_map(&source_map, cm);

    let css_hash = if context.config.hash {
        Some(file_content_hash(&css_code))
    } else {
        None
    };

    Ok(ChunkFile {
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
    create = "{ SizedCache::with_size(100) }",
    key = "u64",
    convert = "{chunk_pot.js_hash}"
)]
fn render_dev_normal_chunk_js_with_cache(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<ChunkFile> {
    let buf: Vec<u8> = chunk_pot.to_chunk_module_content(context).into();

    let hash = if context.config.hash {
        Some(file_content_hash(&buf))
    } else {
        None
    };

    Ok(ChunkFile {
        content: buf,
        hash,
        source_map: vec![],
        file_name: chunk_pot.js_name.clone(),
        chunk_id: chunk_pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[cached(
    result = true,
    type = "SizedCache<u64 , ChunkFile>",
    create = "{ SizedCache::with_size(100) }",
    key = "u64",
    convert = "{chunk_pot.js_hash}"
)]
fn render_normal_chunk_js_with_cache(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<ChunkFile> {
    let module = chunk_pot.to_chunk_module();

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
            stylesheet: stylesheet
                .map(|css_modules| (css_modules.stylesheet, css_modules.raw_hash)),
        })
    }

    pub fn to_dev_normal_chunk_files(&mut self, context: &Arc<Context>) -> Result<Vec<ChunkFile>> {
        let mut files = vec![];

        let js_chunk_file = render_dev_normal_chunk_js_with_cache(self, context)?;

        files.push(js_chunk_file);

        if self.stylesheet.is_some() {
            let css_chunk_file = render_chunk_css(self, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    pub fn to_normal_chunk_files(&mut self, context: &Arc<Context>) -> Result<Vec<ChunkFile>> {
        let mut files = vec![];

        let js_chunk_file = render_normal_chunk_js_with_cache(self, context)?;

        files.push(js_chunk_file);

        if self.stylesheet.is_some() {
            let css_chunk_file = render_chunk_css(self, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    pub fn to_dev_entry_chunk_files(
        &mut self,
        context: &Arc<Context>,
        js_map: &HashMap<String, String>,
        css_map: &HashMap<String, String>,
        chunk: &Chunk,
        full_hash: u64,
    ) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();

        let mut lines = vec![];
        lines.push(format!(
            "var chunksIdToUrlMap= {};",
            serde_json::to_string(js_map).unwrap()
        ));
        lines.push(format!(
            "var cssChunksIdToUrlMap= {};",
            serde_json::to_string(css_map).unwrap()
        ));

        let js_chunk_file = render_dev_entry_chunk_js(self, lines, chunk, context, full_hash)?;

        let mut files = vec![js_chunk_file];

        if self.stylesheet.is_some() {
            mako_core::mako_profile_scope!("CssChunk");
            let css_chunk_file = render_chunk_css(self, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    pub fn to_entry_chunk_files(
        &mut self,
        context: &Arc<Context>,
        js_map: &HashMap<String, String>,
        css_map: &HashMap<String, String>,
        chunk: &Chunk,
        full_hash: u64,
    ) -> Result<Vec<ChunkFile>> {
        let js_chunk_file =
            render_entry_chunk_js(self, js_map, css_map, chunk, context, full_hash)?;

        let mut files = vec![js_chunk_file];

        if self.stylesheet.is_some() {
            let css_chunk_file = render_chunk_css(self, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    fn split_modules<'a>(
        module_ids: &'a IndexSet<ModuleId>,
        module_graph: &'a ModuleGraph,
        context: &'a Arc<Context>,
    ) -> Result<(JsModules<'a>, Option<CssModules>)> {
        mako_core::mako_profile_function!();
        let mut module_map: HashMap<String, (&Module, u64)> = Default::default();
        let mut merged_css_modules: Vec<(String, Stylesheet)> = vec![];

        let mut module_raw_hash_map: HashMap<String, u64> = Default::default();
        let mut css_raw_hashes = vec![];

        let module_ids: Vec<_> = module_ids.iter().collect();

        for module_id in module_ids {
            let module = module_graph.get_module(module_id).unwrap();
            let module_info = module.info.as_ref().unwrap();
            let ast = &module_info.ast;

            if let ModuleAst::Script(_) = ast {
                module_raw_hash_map.insert(module.id.id.clone(), module_info.raw_hash);
            }

            module_map.insert(module.id.generate(context), (module, module_info.raw_hash));

            if let ModuleAst::Css(ast) = ast {
                // only apply the last css module if chunk depend on it multiple times
                // make sure the rules order is correct
                if let Some(index) = merged_css_modules
                    .iter()
                    .position(|(id, _)| id.eq(&module.id.id))
                {
                    merged_css_modules.remove(index);
                }
                merged_css_modules.push((module.id.id.clone(), ast.clone()));
                css_raw_hashes.push(module_info.raw_hash);
            }
        }

        let raw_hash = hash_map_ordered_by_key(&module_raw_hash_map);

        if !merged_css_modules.is_empty() {
            mako_core::mako_profile_scope!("iter_chunk_css_modules");

            let mut stylesheet = Stylesheet {
                span: DUMMY_SP,
                rules: vec![],
            };

            for (_, ast) in merged_css_modules {
                stylesheet.rules.extend(ast.rules);
            }

            let css_raw_hash = hash_vec(&css_raw_hashes);

            Ok((
                JsModules {
                    module_map,
                    raw_hash,
                },
                Some(CssModules {
                    stylesheet,
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

    pub fn to_module_object(&self) -> ObjectLit {
        let mut sorted_kv = self
            .module_map
            .iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<_>>();
        sorted_kv.sort_by_key(|(k, _)| *k);

        let mut props = Vec::new();

        for (module_id_str, module) in sorted_kv {
            let fn_expr = module.0.to_module_fn_expr().unwrap().unwrap();

            let pv: PropOrSpread = Prop::KeyValue(KeyValueProp {
                key: quote_str!(module_id_str.clone()).into(),
                value: fn_expr.into(),
            })
            .into();

            props.push(pv);
        }

        ObjectLit {
            span: DUMMY_SP,
            props,
        }
    }

    fn to_chunk_module_object_string(&self, context: &Arc<Context>) -> String {
        mako_core::mako_profile_function!();

        let sorted_kv = {
            mako_core::mako_profile_scope!("collect_&_sort");

            let mut sorted_kv = self
                .module_map
                .iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>();

            if context.config.hash {
                sorted_kv.sort_by_key(|(k, _)| *k);
            }

            sorted_kv
        };

        let module_defines = sorted_kv
            .par_iter()
            .map(|(module_id_str, module_and_hash)| {
                to_module_line(module_and_hash.0, context, module_and_hash.1, module_id_str)
            })
            .collect::<Vec<String>>()
            .join("\n");

        {
            mako_core::mako_profile_scope!("wrap_in_brace");
            format!(r#"{{ {} }}"#, module_defines)
        }
    }

    pub fn to_chunk_module_content(&self, context: &Arc<Context>) -> String {
        format!(
            r#"globalThis.jsonpCallback([["{}"],
{}]);"#,
            self.chunk_id,
            self.to_chunk_module_object_string(context)
        )
    }

    pub fn to_chunk_module(&self) -> SwcModule {
        // key: module id
        // value: module FnExpr
        let mut sorted_kv = self
            .module_map
            .iter()
            .map(|(k, v)| (k, v))
            .collect::<Vec<_>>();

        sorted_kv.sort_by_key(|(k, _)| *k);

        let mut props = Vec::new();

        sorted_kv
            .into_iter()
            .for_each(|(module_id_str, module_hash_tuple)| {
                let fn_expr = module_hash_tuple.0.to_module_fn_expr().unwrap().unwrap();

                let pv: PropOrSpread = Prop::KeyValue(KeyValueProp {
                    key: quote_str!(module_id_str.clone()).into(),
                    value: fn_expr.into(),
                })
                .into();

                props.push(pv);
            });

        let module_object = self.to_module_object();

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

        SwcModule {
            body: vec![jsonp_callback_stmt.into()],
            shebang: None,
            span: DUMMY_SP,
        }
    }
}

#[cached(
    key = "String",
    type = "SizedCache<String , String>",
    create = "{ SizedCache::with_size(2000) }",
    convert = r#"{format!("{}-{}", _raw_hash, module_id_str)}"#
)]
fn to_module_line(
    fn_expr: &Module,
    context: &Arc<Context>,
    _raw_hash: u64, // used for cache key
    module_id_str: &str,
) -> String {
    mako_core::mako_profile_function!(module_id_str);

    let _now = Instant::now();

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

    let escaped = match &fn_expr.info.as_ref().unwrap().ast {
        ModuleAst::Script(ast) => {
            emitter.emit_module(&ast.ast).unwrap();

            let source_map = build_source_map(&source_map_buf, &cm);

            let content = String::from_utf8_lossy(&buf);

            let content = vec![
                content,
                format!(
                    "//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                    base64_encode(source_map)
                )
                .into(),
            ]
            .join("");

            serde_json::to_string(&content).unwrap()
        }
        ModuleAst::Css(_) => "".to_string(),
        ModuleAst::None => {
            panic!("xxx")
        }
    };

    format!(
        r#""{}" : function (module, exports, require){{
    eval({})
  }},"#,
        module_id_str, escaped
    )
}

struct JsModules<'a> {
    pub module_map: HashMap<String, (&'a Module, u64)>,
    raw_hash: u64,
}

struct CssModules {
    stylesheet: Stylesheet,
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
    type = "SizedCache<String, ChunkFile>",
    create = "{ SizedCache::with_size(10) }",
    convert = r#"{format!("{}-{}",pot.js_hash, full_hash)}"#
)]
fn render_dev_entry_chunk_js(
    pot: &ChunkPot,
    mut stmts: Vec<String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    full_hash: u64,
) -> Result<ChunkFile> {
    mako_core::mako_profile_function!();

    let chunk_graph = context.chunk_graph.read().unwrap();

    let dep_chunks_ids = chunk_graph
        .sync_dependencies_chunk(chunk)
        .into_iter()
        .map(|chunk| chunk.generate(context))
        .collect::<HashSet<String>>()
        .into_iter()
        .collect::<Vec<_>>();

    // var d = [ ..all dep chunk ids ]
    let dep_chunk_ids_decl_stmt = format!("var d= [{}];", dep_chunks_ids.join(","));
    stmts.push(dep_chunk_ids_decl_stmt);

    // var cssInstalledChunks = { "chunk_id": 0 }
    let init_install_css_chunk = format!(
        r#"var cssInstalledChunks = {{ "{}" : 0 }};"#,
        pot.chunk_id.clone()
    );

    stmts.push(init_install_css_chunk);
    stmts.push(format!("var e = \"{}\";", pot.chunk_id));

    let runtime_content =
        runtime_base_code(context)?.replace("_%full_hash%_", &full_hash.to_string());

    let mut content: Vec<u8> =
        format!("var m = {};", pot.to_chunk_module_object_string(context)).into();

    {
        mako_core::mako_profile_scope!("assemble");
        content.extend(stmts.join("\n").into_bytes());
        content.extend(runtime_content.into_bytes());
    }
    Ok(ChunkFile {
        content,
        hash: None,
        source_map: vec![],
        file_name: pot.js_name.clone(),
        chunk_id: pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{format!("{}-{}",pot.js_hash, full_hash)}"#
)]
fn render_entry_chunk_js(
    pot: &ChunkPot,
    js_map: &HashMap<String, String>,
    css_map: &HashMap<String, String>,
    chunk: &Chunk,
    context: &Arc<Context>,
    full_hash: u64,
) -> Result<ChunkFile> {
    let mut stmts = vec![];

    let chunk_graph = context.chunk_graph.read().unwrap();

    let dep_chunks_ids = chunk_graph
        .sync_dependencies_chunk(chunk)
        .into_iter()
        .map(|chunk| chunk.generate(context))
        .collect::<HashSet<String>>()
        .into_iter()
        .map(|id| Some(quote_str!(id).as_arg()))
        .collect::<Vec<_>>();

    // var d = [ ..all dep chunk ids ]
    let dep_chunk_ids_decl_stmt: Stmt = ArrayLit {
        span: DUMMY_SP,
        elems: dep_chunks_ids,
    }
    .into_var_decl(VarDeclKind::Var, quote_ident!("d").into())
    .into();

    stmts.push(dep_chunk_ids_decl_stmt);

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

    let (js_map_stmt, css_map_stmt) = chunk_map_decls(js_map, css_map);

    stmts.push(js_map_stmt);
    stmts.push(css_map_stmt);

    if let ChunkType::Entry(module_id, _) = &chunk.chunk_type {
        let main_id_decl: Stmt = quote_str!(module_id.generate(context))
            .into_var_decl(VarDeclKind::Var, quote_ident!("e").into()) // e brief for entry_module_id
            .into();

        stmts.push(main_id_decl);
    }

    let runtime_content =
        runtime_base_code(context)?.replace("_%full_hash%_", &full_hash.to_string());

    let mut ast = build_js_ast(
        "_mako_internal/runtime_entry.js",
        runtime_content.as_str(),
        context,
    )
    .unwrap();

    let modules_lit: Stmt = pot
        .to_module_object()
        .into_var_decl(VarDeclKind::Var, quote_ident!("m").into())
        .into();

    ast.ast.body.insert(0, modules_lit.into());

    ast.ast
        .body
        .splice(0..0, stmts.into_iter().map(|s| s.into()));

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_js(&mut ast, context)?;
    }

    let (buf, source_map_buf) = render_module_js(&ast.ast, context)?;

    let hash = if context.config.hash {
        Some(file_content_hash(&buf))
    } else {
        None
    };

    Ok(ChunkFile {
        content: buf,
        hash,
        source_map: source_map_buf,
        file_name: pot.js_name.clone(),
        chunk_id: pot.chunk_id.clone(),
        file_type: ChunkFileType::JS,
    })
}

#[once(result = true)]
fn runtime_base_code(context: &Arc<Context>) -> Result<String> {
    let runtime_entry_content_str = include_str!("runtime/runtime_entry.js");
    let content = runtime_entry_content_str.replace(
        "// __inject_runtime_code__",
        &context.plugin_driver.runtime_plugins_code(context)?,
    );

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
