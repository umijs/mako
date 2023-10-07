use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::vec;

use anyhow::Result;
use indexmap::IndexSet;
use rayon::prelude::*;
use swc_common::{BytePos, LineCol, DUMMY_SP};
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

use crate::ast::build_js_ast;
use crate::chunk::{Chunk, ChunkType};
use crate::compiler::{Compiler, Context};
use crate::config::Mode;
use crate::generate::hash_file_name;
use crate::load::file_content_hash;
use crate::minify::{minify_css, minify_js};
use crate::module::{ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;
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
    pub stylesheet: Option<Stylesheet>,
}

impl ChunkPot {
    pub fn into_chunk_module(&self) -> SwcModule {
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
        let (module_map, stylesheet) =
            ChunkPot::modules_to_js_stmts(chunk.get_modules(), mg, context)?;

        Ok(ChunkPot {
            js_name: chunk.filename(),
            chunk_id: chunk.id.generate(context),
            module_map,
            stylesheet,
        })
    }

    fn modules_to_js_stmts(
        module_ids: &IndexSet<ModuleId>,
        module_graph: &ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<(HashMap<String, FnExpr>, Option<Stylesheet>)> {
        let mut module_map: HashMap<String, FnExpr> = Default::default();
        let mut merged_css_modules: Vec<(String, Stylesheet)> = vec![];

        let module_ids: Vec<_> = module_ids.iter().collect();

        for module_id in module_ids {
            let module = module_graph.get_module(module_id).unwrap();
            let ast = module.info.as_ref().unwrap();
            let ast = &ast.ast;

            let m = module.as_module_fn_expr()?;

            if let Some(fn_expr) = m {
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
            Ok((module_map, Some(merged_css_ast)))
        } else {
            Ok((module_map, None))
        }
    }
}

pub enum ChunkFileType {
    JS,
    CSS,
}

pub struct ChunkFile {
    pub content: Vec<u8>,
    pub source_map: Vec<(BytePos, LineCol)>,
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
    pub fn generate_chunks_ast(&self) -> Result<Vec<ChunkFile>> {
        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let chunks = chunk_graph.get_chunks();

        let chunk_files = chunks
            .par_iter()
            .filter(|chunk| !matches!(chunk.chunk_type, ChunkType::Entry(_, _)))
            .map(|chunk| {
                // build stmts
                let mut pot: ChunkPot = ChunkPot::from(chunk, &module_graph, &self.context)?;

                self.render_normal_chunk(&mut pot)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let mut js_chunk_props: Vec<PropOrSpread> = vec![];
        let mut css_chunk_map: Vec<PropOrSpread> = vec![];

        for f in chunk_files.iter() {
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

        let init_install_css_chunk = |chunk_id: &String| -> Stmt {
            let prop = Prop::KeyValue(KeyValueProp {
                key: quote_str!(chunk_id.clone()).into(),
                value: Lit::Num(Number {
                    span: DUMMY_SP,
                    value: 0f64,
                    raw: None,
                })
                .into(),
            });

            ObjectLit {
                span: DUMMY_SP,
                props: vec![prop.into()],
            }
            .into_var_decl(VarDeclKind::Var, quote_ident!("installedChunks").into())
            .into()
        };

        let mut entry_chunk_files = chunks
            .par_iter()
            .filter(|chunk| matches!(chunk.chunk_type, ChunkType::Entry(_, _)))
            .map(|&chunk| {
                let mut pot = ChunkPot::from(chunk, &module_graph, &self.context)?;

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

                let mut before_stmts = vec![
                    modules_lit,
                    js_chunk_map_dcl_stmt.clone(),
                    css_chunk_map_dcl_stmt.clone(),
                    init_install_css_chunk(&pot.chunk_id),
                ];

                if let ChunkType::Entry(module_id, _) = &chunk.chunk_type {
                    let main_id_decl: Stmt = quote_str!(module_id.generate(&self.context))
                        .into_var_decl(VarDeclKind::Var, quote_ident!("e").into())
                        .into();

                    before_stmts.push(main_id_decl);
                }

                self.render_entry_chunk(&mut pot, before_stmts, chunk)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        entry_chunk_files.extend(chunk_files);

        Ok(entry_chunk_files)
    }

    fn render_entry_chunk(
        &self,
        pot: &mut ChunkPot,
        mut stmts: Vec<Stmt>,
        chunk: &Chunk,
    ) -> Result<Vec<ChunkFile>> {
        let full_hash = self.full_hash();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let dep_chunks_ids = chunk_graph
            .sync_dependencies_chunk(chunk)
            .into_iter()
            .map(|chunk| chunk.generate(&self.context))
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

        let runtime_content = compile_runtime_entry(
            self.context
                .assets_info
                .lock()
                .unwrap()
                .values()
                .any(|info| info.ends_with(".wasm")),
            self.context
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
            &self
                .context
                .plugin_driver
                .runtime_plugins_code(&self.context)?,
        );

        let mut ast = build_js_ast(
            "mako_internal_runtime_entry.js",
            runtime_content.as_str(),
            &self.context,
        )
        .unwrap();

        ast.ast
            .body
            .splice(0..0, stmts.into_iter().map(|s| s.into()));

        let mut files = vec![];

        if self.context.config.minify && matches!(self.context.config.mode, Mode::Production) {
            minify_js(&mut ast, &self.context)?;
        }

        let (code, source_map) = self.render_js(&ast.ast)?;

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

        if let Some(stylesheet) = &mut pot.stylesheet {
            let (code, source_map) = self.render_css(stylesheet)?;

            files.push(ChunkFile {
                chunk_id: pot.chunk_id.clone(),
                file_name: get_css_chunk_filename(&pot.js_name),
                source_map,
                file_type: ChunkFileType::CSS,
                hash: if self.context.config.hash {
                    Some(file_content_hash(&code))
                } else {
                    None
                },
                content: code,
            });
        }

        Ok(files)
    }

    fn render_normal_chunk(&self, pot: &mut ChunkPot) -> Result<Vec<ChunkFile>> {
        let mut files = vec![];

        let module = pot.into_chunk_module();

        let (code, source_map) = self.render_js(&module)?;

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

        if let Some(style) = &mut pot.stylesheet {
            let (css_code, css_source_map) = self.render_css(style)?;

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

    fn render_css(&self, ast: &mut Stylesheet) -> Result<(Vec<u8>, Vec<(BytePos, LineCol)>)> {
        let context = &self.context;
        let mut css_code = String::new();
        let mut source_map = Vec::new();
        let css_writer = BasicCssWriter::new(
            &mut css_code,
            Some(&mut source_map),
            BasicCssWriterConfig::default(),
        );

        if self.context.config.minify && matches!(self.context.config.mode, Mode::Production) {
            minify_css(ast, &self.context)?;
        }

        let mut gen = CodeGenerator::new(
            css_writer,
            CodegenConfig {
                minify: context.config.minify && matches!(context.config.mode, Mode::Production),
            },
        );
        gen.emit(ast)?;

        Ok((css_code.into(), source_map))
    }

    fn render_js(&self, ast: &SwcModule) -> Result<(Vec<u8>, Vec<(BytePos, LineCol)>)> {
        let mut buf = vec![];
        let mut source_map_buf = Vec::new();
        let context = &self.context;
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
        emitter.emit_module(ast)?;

        Ok((buf, source_map_buf))
    }
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
