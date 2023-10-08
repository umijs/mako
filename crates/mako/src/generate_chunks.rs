use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::Arc;
use std::vec;

use anyhow::Result;
use cached::proc_macro::cached;
use indexmap::IndexSet;
use rayon::prelude::*;
use swc_common::DUMMY_SP;
use swc_css_ast::Stylesheet;
use swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use swc_ecma_ast::{
    ArrayLit, Expr, ExprOrSpread, FnExpr, KeyValueProp, Lit, Module as SwcModule, Number,
    ObjectLit, Prop, PropOrSpread, Stmt, Str, VarDeclKind,
};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use swc_ecma_utils::{member_expr, quote_ident, quote_str, ExprFactory};
use twox_hash::XxHash64;

use crate::ast::build_js_ast;
use crate::chunk::{Chunk, ChunkType};
use crate::compiler::{Compiler, Context};
use crate::config::Mode;
use crate::generate::hash_file_name;
use crate::load::file_content_hash;
use crate::minify::{minify_css, minify_js};
use crate::module::{ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;
use crate::sourcemap::build_source_map;
use crate::transform_in_generate::transform_css_generate;

pub struct OutputAst {
    pub path: String,
    pub ast: ModuleAst,
    pub chunk_id: String,
    pub ast_module_hash: u64,
}

pub struct ChunkPot {
    pub chunk_id: String,
    pub js_name: String,
    pub module_map: HashMap<String, FnExpr>,
    pub js_hash: u64,
    pub stylesheet: Option<(Stylesheet, u64)>,
}

impl ChunkPot {
    pub fn to_chunk_module(&self) -> SwcModule {
        // key: module id
        // value: module FnExpr
        let props = self
            .module_map
            .iter()
            .map(|(module_id_str, fn_expr)| {
                Prop::KeyValue(KeyValueProp {
                    key: quote_str!(module_id_str.clone()).into(),
                    value: fn_expr.clone().into(),
                })
                .into()
            })
            .collect::<Vec<PropOrSpread>>();

        let module_object = ObjectLit {
            span: DUMMY_SP,
            props,
        };

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

impl ChunkPot {
    pub fn from(chunk: &Chunk, mg: &ModuleGraph, context: &Arc<Context>) -> Result<Self> {
        let ((module_map, js_hash), stylesheet) =
            ChunkPot::modules_to_js_stmts(chunk.get_modules(), mg, context)?;

        Ok(ChunkPot {
            js_name: chunk.filename(),
            chunk_id: chunk.id.generate(context),
            module_map,
            js_hash,
            stylesheet,
        })
    }

    fn modules_to_js_stmts(
        module_ids: &IndexSet<ModuleId>,
        module_graph: &ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<((HashMap<String, FnExpr>, u64), Option<(Stylesheet, u64)>)> {
        let mut module_map: HashMap<String, FnExpr> = Default::default();
        let mut merged_css_modules: Vec<(String, Stylesheet)> = vec![];

        let mut module_raw_hash_map: HashMap<String, u64> = Default::default();
        let mut css_raw_hashes = vec![];

        let module_ids: Vec<_> = module_ids.iter().collect();

        for module_id in module_ids {
            let module = module_graph.get_module(module_id).unwrap();
            let module_info = module.info.as_ref().unwrap();
            let ast = &module_info.ast;

            let m = module.as_module_fn_expr()?;

            if let Some(fn_expr) = m {
                module_raw_hash_map.insert(module.id.id.clone(), module_info.raw_hash);
                module_map.insert(module.id.generate(context), fn_expr);
            }

            if let ModuleAst::Css(ast) = ast {
                // only apply the last css module if chunk depend on it multiple times
                // make sure the rules order is correct
                if let Some(index) = merged_css_modules
                    .iter()
                    .position(|(id, _)| id.eq(&module.id.id))
                {
                    merged_css_modules.remove(index);
                    css_raw_hashes.remove(index);
                }
                merged_css_modules.push((module.id.id.clone(), ast.clone()));
                css_raw_hashes.push(module_info.raw_hash);
            }
        }

        let js_hash = hash_map_ordered_by_key(&module_raw_hash_map);

        if !merged_css_modules.is_empty() {
            let mut merged_css_ast = Stylesheet {
                span: DUMMY_SP,
                rules: vec![],
            };

            for (_, ast) in merged_css_modules {
                merged_css_ast.rules.extend(ast.rules);
            }

            transform_css_generate(&mut merged_css_ast, context);

            let css_hash = hash_vec(&css_raw_hashes);

            Ok(((module_map, js_hash), Some((merged_css_ast, css_hash))))
        } else {
            Ok(((module_map, js_hash), None))
        }
    }
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

pub enum ChunkFileType {
    JS,
    CSS,
}

pub struct ChunkFile {
    pub content: Vec<u8>,
    pub source_map: Vec<u8>,
    pub hash: Option<String>,
    pub file_name: String,
    pub chunk_id: String,
    pub file_type: ChunkFileType,
}

impl ChunkFile {
    pub fn disk_name(&self) -> String {
        if let Some(hash) = &self.hash {
            hash_file_name(&self.file_name, hash)
        } else {
            self.file_name.clone()
        }
    }

    pub fn source_map_disk_name(&self) -> String {
        format!("{}.map", self.disk_name())
    }

    pub fn source_map_name(&self) -> String {
        format!("{}.map", self.file_name)
    }
}

impl Compiler {
    pub fn generate_chunk_files(&self) -> Result<Vec<ChunkFile>> {
        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let chunks = chunk_graph.get_chunks();

        let non_entry_chunk_files = self.generate_non_entry_chunk_files()?;

        let (js_chunk_map_dcl_stmt, css_chunk_map_dcl_stmt) =
            Self::chunk_map_decls(&non_entry_chunk_files);

        let mut entry_chunk_files = chunks
            .par_iter()
            .filter(|chunk| matches!(chunk.chunk_type, ChunkType::Entry(_, _)))
            .map(|&chunk| {
                let mut pot = ChunkPot::from(chunk, &module_graph, &self.context)?;

                let mut before_stmts = vec![
                    js_chunk_map_dcl_stmt.clone(),
                    css_chunk_map_dcl_stmt.clone(),
                ];

                if let ChunkType::Entry(module_id, _) = &chunk.chunk_type {
                    let main_id_decl: Stmt = quote_str!(module_id.generate(&self.context))
                        .into_var_decl(VarDeclKind::Var, quote_ident!("e").into())
                        .into();

                    before_stmts.push(main_id_decl);
                }

                self.to_entry_chunk_files(&mut pot, before_stmts, chunk)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        entry_chunk_files.extend(non_entry_chunk_files);

        Ok(entry_chunk_files)
    }

    fn generate_non_entry_chunk_files(&self) -> Result<Vec<ChunkFile>> {
        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let chunks = chunk_graph.get_chunks();

        let fs = chunks
            .par_iter()
            .filter(|chunk| !matches!(chunk.chunk_type, ChunkType::Entry(_, _)))
            .map(|chunk| {
                // build stmts
                let mut pot: ChunkPot = ChunkPot::from(chunk, &module_graph, &self.context)?;

                self.to_normal_chunk_files(&mut pot)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok(fs)
    }

    fn chunk_map_decls(non_entry_chunk_files: &[ChunkFile]) -> (Stmt, Stmt) {
        let mut js_chunk_props: Vec<PropOrSpread> = vec![];
        let mut css_chunk_map: Vec<PropOrSpread> = vec![];

        for f in non_entry_chunk_files.iter() {
            let prop_kv = Prop::KeyValue(KeyValueProp {
                key: quote_str!(f.chunk_id.clone()).into(),
                value: quote_str!(f.disk_name()).into(),
            });

            match f.file_type {
                ChunkFileType::JS => {
                    js_chunk_props.push(prop_kv.into());
                }
                ChunkFileType::CSS => {
                    css_chunk_map.push(prop_kv.into());
                }
            }
        }

        let js_chunk_map_dcl_stmt: Stmt = ObjectLit {
            span: DUMMY_SP,
            props: js_chunk_props,
        }
        .into_var_decl(VarDeclKind::Var, quote_ident!("chunksIdToUrlMap").into())
        .into();

        let css_chunk_map_dcl_stmt: Stmt = ObjectLit {
            span: DUMMY_SP,
            props: css_chunk_map,
        }
        .into_var_decl(VarDeclKind::Var, quote_ident!("cssChunksIdToUrlMap").into())
        .into();

        (js_chunk_map_dcl_stmt, css_chunk_map_dcl_stmt)
    }

    fn to_entry_chunk_files(
        &self,
        pot: &mut ChunkPot,
        stmts: Vec<Stmt>,
        chunk: &Chunk,
    ) -> Result<Vec<ChunkFile>> {
        let full_hash = self.full_hash();
        let mut files = vec![];

        let (code, source_map) =
            render_entry_chunk_js(pot, stmts, chunk, &self.context, full_hash)?;

        files.push(ChunkFile {
            chunk_id: pot.chunk_id.clone(),
            file_name: pot.js_name.clone(),
            source_map,
            file_type: ChunkFileType::JS,
            hash: if self.context.config.hash {
                Some(file_content_hash(&code))
            } else {
                None
            },
            content: code,
        });

        if pot.stylesheet.is_some() {
            let (css_code, css_source_map) = render_chunk_css(pot, &self.context)?;

            files.push(ChunkFile {
                chunk_id: pot.chunk_id.clone(),
                file_name: get_css_chunk_filename(&pot.js_name),
                source_map: css_source_map,
                file_type: ChunkFileType::CSS,
                hash: if self.context.config.hash {
                    Some(file_content_hash(&css_code))
                } else {
                    None
                },
                content: css_code,
            });
        }

        Ok(files)
    }

    fn to_normal_chunk_files(&self, pot: &mut ChunkPot) -> Result<Vec<ChunkFile>> {
        let mut files = vec![];

        let (code, source_map) = render_normal_chunk_js(pot, &self.context)?;

        let hash = if self.context.config.hash {
            Some(file_content_hash(&code))
        } else {
            None
        };

        files.push(ChunkFile {
            content: code,
            hash,
            source_map,
            file_name: pot.js_name.clone(),
            chunk_id: pot.chunk_id.clone(),
            file_type: ChunkFileType::JS,
        });

        if pot.stylesheet.is_some() {
            let (css_code, css_source_map) = render_chunk_css(pot, &self.context)?;

            let css_hash = if self.context.config.hash {
                Some(file_content_hash(&css_code))
            } else {
                None
            };

            files.push(ChunkFile {
                content: css_code,
                hash: css_hash,
                source_map: css_source_map,
                file_name: get_css_chunk_filename(&pot.js_name),
                chunk_id: pot.chunk_id.clone(),
                file_type: ChunkFileType::CSS,
            });
        }

        Ok(files)
    }
}

// TODO：
//  entry chunk 缓存需要重新设计
//  或者 entry chunk 在 dev 阶段需要尽量的小

#[cached(
    result = true,
    key = "String",
    convert = r#"{format!("{}-{}",pot.js_hash, full_hash)}"#
)]
fn render_entry_chunk_js(
    pot: &mut ChunkPot,
    mut stmts: Vec<Stmt>,
    chunk: &Chunk,
    context: &Arc<Context>,
    full_hash: u64,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let chunk_graph = context.chunk_graph.read().unwrap();

    let dep_chunks_ids = chunk_graph
        .sync_dependencies_chunk(chunk)
        .into_iter()
        .map(|chunk| chunk.generate(context))
        .collect::<HashSet<String>>()
        .into_iter()
        .map(|id| Some(quote_str!(id).as_arg()))
        .collect::<Vec<_>>();

    let dep_chunk_ids_decl_stmt: Stmt = ArrayLit {
        span: DUMMY_SP,
        elems: dep_chunks_ids,
    }
    .into_var_decl(VarDeclKind::Var, quote_ident!("d").into())
    .into();

    stmts.push(dep_chunk_ids_decl_stmt);

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

    let runtime_content = compile_runtime_entry(
        context
            .assets_info
            .lock()
            .unwrap()
            .values()
            .any(|info| info.ends_with(".wasm")),
        context
            .module_graph
            .read()
            .unwrap()
            .modules()
            .iter()
            .any(|module| module.info.as_ref().unwrap().is_async),
    )
    .replace("_%full_hash%_", &full_hash.to_string())
    .replace(
        "// __inject_runtime_code__",
        &context.plugin_driver.runtime_plugins_code(context)?,
    );

    let mut ast = build_js_ast(
        "mako_internal_runtime_entry.js",
        runtime_content.as_str(),
        context,
    )
    .unwrap();

    let mut module_props: Vec<PropOrSpread> = vec![];

    pot.module_map.iter().for_each(|(module_id, fn_expr)| {
        let prop_kv = Prop::KeyValue(KeyValueProp {
            key: quote_str!(module_id.clone()).into(),
            value: fn_expr.clone().into(),
        });
        module_props.push(prop_kv.into());
    });

    let modules_lit: Stmt = ObjectLit {
        span: DUMMY_SP,
        props: module_props,
    }
    .into_var_decl(VarDeclKind::Var, quote_ident!("m").into())
    .into();

    ast.ast.body.insert(0, modules_lit.into());

    ast.ast
        .body
        .splice(0..0, stmts.into_iter().map(|s| s.into()));

    if context.config.minify && matches!(context.config.mode, Mode::Production) {
        minify_js(&mut ast, context)?;
    }

    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();

    let mut emitter = Emitter {
        cfg: JsCodegenConfig {
            minify: context.config.minify && matches!(context.config.mode, Mode::Production),
            target: context.config.output.es_version,
            // ascii_only: true, not working with lodash
            ..Default::default()
        },
        cm: cm.clone(),
        comments: Some(swc_comments),
        wr: Box::new(JsWriter::new(cm, "\n", &mut buf, Some(&mut source_map_buf))),
    };
    emitter.emit_module(&ast.ast)?;

    let cm = &context.meta.script.cm;
    let source_map_buf = build_source_map(&source_map_buf, cm);

    Ok((buf, source_map_buf))
}

#[cached(result = true, key = "u64", convert = "{chunk_pot.js_hash}")]
fn render_normal_chunk_js(
    chunk_pot: &ChunkPot,
    context: &Arc<Context>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();

    let ast = chunk_pot.to_chunk_module();

    let mut emitter = Emitter {
        cfg: JsCodegenConfig {
            minify: context.config.minify && matches!(context.config.mode, Mode::Production),
            target: context.config.output.es_version,
            // ascii_only: true, not working with lodash
            ..Default::default()
        },
        cm: cm.clone(),
        comments: Some(swc_comments),
        wr: Box::new(JsWriter::new(cm, "\n", &mut buf, Some(&mut source_map_buf))),
    };
    emitter.emit_module(&ast)?;

    let cm = &context.meta.script.cm;
    let source_map_buf = build_source_map(&source_map_buf, cm);

    Ok((buf, source_map_buf))
}

#[cached(
    result = true,
    key = "u64",
    convert = "{chunk_pot.stylesheet.as_ref().unwrap().1}"
)]
fn render_chunk_css(
    chunk_pot: &mut ChunkPot,
    context: &Arc<Context>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut css_code = String::new();
    let mut source_map = Vec::new();
    let css_writer = BasicCssWriter::new(
        &mut css_code,
        Some(&mut source_map),
        BasicCssWriterConfig::default(),
    );

    let ast = &mut chunk_pot.stylesheet.as_mut().unwrap().0;

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

    Ok((css_code.into(), source_map))
}

fn get_css_chunk_filename(js_chunk_filename: &String) -> String {
    format!(
        "{}.css",
        js_chunk_filename.strip_suffix(".js").unwrap_or("")
    )
}

fn compile_runtime_entry(has_wasm: bool, has_async: bool) -> String {
    let runtime_entry_content_str = include_str!("runtime/runtime_entry.js");
    runtime_entry_content_str
        .replace(
            "// __WASM_REQUIRE_SUPPORT",
            if has_wasm {
                include_str!("runtime/runtime_wasm.js")
            } else {
                ""
            },
        )
        .replace(
            "// __REQUIRE_ASYNC_MODULE_SUPPORT",
            if has_async {
                include_str!("runtime/runtime_async.js")
            } else {
                ""
            },
        )
}

pub fn build_props(key_str: &str, value: Box<Expr>) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: swc_ecma_ast::PropName::Str(Str {
            span: DUMMY_SP,
            value: key_str.into(),
            raw: None,
        }),
        value,
    })))
}

pub fn modules_to_js_stmts(
    module_ids: &IndexSet<ModuleId>,
    module_graph: &std::sync::RwLockReadGuard<crate::module_graph::ModuleGraph>,
    context: &Arc<Context>,
) -> Result<(Vec<PropOrSpread>, Option<Stylesheet>)> {
    let mut js_stmts = vec![];
    let mut merged_css_modules: Vec<(String, Stylesheet)> = vec![];

    let module_ids: Vec<_> = module_ids.iter().collect();

    for module_id in module_ids {
        let module = module_graph.get_module(module_id).unwrap();
        let ast = module.info.as_ref().unwrap();
        let ast = &ast.ast;

        let m = module.as_module_fn_expr()?;

        if let Some(fn_expr) = m {
            js_stmts.push(build_props(
                module.id.generate(context).as_str(),
                fn_expr.into(),
            ))
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
            merged_css_modules.push((module.id.id.clone(), ast.clone()));
        }
    }
    if !merged_css_modules.is_empty() {
        let mut merged_css_ast = Stylesheet {
            span: DUMMY_SP,
            rules: vec![],
        };

        for (_, ast) in merged_css_modules {
            merged_css_ast.rules.extend(ast.rules);
        }

        transform_css_generate(&mut merged_css_ast, context);
        Ok((js_stmts, Some(merged_css_ast)))
    } else {
        Ok((js_stmts, None))
    }
}

fn to_array_lit(elems: Vec<ExprOrSpread>) -> ArrayLit {
    ArrayLit {
        span: DUMMY_SP,
        elems: elems.into_iter().map(Some).collect::<Vec<_>>(),
    }
}
