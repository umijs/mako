use std::sync::Arc;
use std::vec;

use anyhow::{anyhow, Result};
use indexmap::IndexSet;
use rayon::prelude::*;
use swc_common::DUMMY_SP;
use swc_css_ast::Stylesheet;
use swc_ecma_ast::{
    ArrayLit, BindingIdent, BlockStmt, CallExpr, Callee, Decl, Expr, ExprOrSpread, ExprStmt,
    FnExpr, Function, Ident, KeyValueProp, MemberExpr, MemberProp, ModuleItem, ObjectLit, Param,
    Pat, Prop, PropOrSpread, Stmt, Str, VarDecl,
};

use crate::ast::build_js_ast;
use crate::compiler::{Compiler, Context};
use crate::module::{ModuleAst, ModuleId};
use crate::transform_in_generate::transform_css_generate;

pub struct OutputAst {
    pub path: String,
    pub ast: ModuleAst,
    pub chunk_id: String,
    pub ast_module_hash: u64,
}

impl Compiler {
    pub fn generate_chunks_ast(&self) -> Result<Vec<OutputAst>> {
        let full_hash = self.full_hash();

        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.write().unwrap();

        let use_hash = self.context.config.hash;

        let chunks = chunk_graph.get_chunks();
        // TODO: remove this
        let chunks_map_str: Vec<String> = chunks
            .iter()
            .map(|chunk| {
                format!(
                    "chunksIdToUrlMap[\"{}\"] = `{}`;",
                    chunk.id.generate(&self.context),
                    if use_hash {
                        chunk.js_chunk_name_with_hash(&module_graph)
                    } else {
                        chunk.filename()
                    }
                )
            })
            .collect();
        let chunks_map_str = format!(
            "const chunksIdToUrlMap = {{}};\n{}",
            chunks_map_str.join("\n")
        );

        let css_chunks_map_str: Vec<String> = {
            chunks
                .iter()
                .filter_map(|chunk| match chunk.chunk_type {
                    crate::chunk::ChunkType::Async
                    | crate::chunk::ChunkType::Entry(_, _)
                    | crate::chunk::ChunkType::Sync => {
                        let module_ids = chunk.get_modules();
                        let module_ids: Vec<_> = module_ids.iter().collect();

                        for module_id in module_ids {
                            let module = module_graph.get_module(module_id).unwrap();

                            if let Some(info) = module.info.as_ref() {
                                match &info.ast {
                                    ModuleAst::Css(_) => {
                                        let str = format!(
                                            "cssChunksIdToUrlMap[\"{}\"] = `{}`;",
                                            chunk.id.generate(&self.context),
                                            if use_hash {
                                                chunk.css_chunk_name_with_hash(&module_graph)
                                            } else {
                                                get_css_chunk_filename(chunk.filename())
                                            }
                                        );

                                        match chunk.chunk_type {
                                            crate::chunk::ChunkType::Entry(_, _) => {
                                                return Some(format!(
                                                    "installedChunks['{}'] = 0;\n{}",
                                                    chunk.id.generate(&self.context),
                                                    str,
                                                ))
                                            }
                                            _ => return Some(str),
                                        }
                                    }
                                    _ => continue,
                                }
                            }
                        }
                        None
                    }
                    _ => None,
                })
                .collect()
        };
        let css_chunks_map_str = css_chunks_map_str.join("\n");

        let chunks_ast = chunks
            .par_iter()
            .map(|chunk| {
                // build stmts
                let module_ids = chunk.get_modules();

                let stmts_res = modules_to_js_stmts(module_ids, &module_graph, &self.context);

                if stmts_res.is_err() {
                    return Err(anyhow!(
                        "Chunk {} failed to generate js ast {:?}",
                        chunk.id.id,
                        stmts_res.err().unwrap()
                    ));
                }

                let (js_stmts, merged_css_ast) = stmts_res.unwrap();

                // build js ast
                let content = match &chunk.chunk_type {
                    crate::chunk::ChunkType::Entry(module_id, _) => {
                        let chunks_ids = chunk_graph
                            .sync_dependencies_chunk(chunk)
                            .into_iter()
                            .map(|chunk| chunk.generate(&self.context))
                            .collect::<Vec<String>>();

                        let code = format!(
                            "{}\n{}",
                            chunks_map_str,
                            compile_runtime_entry(
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
                        )
                        .replace("_%full_hash%_", &full_hash.to_string())
                        .replace(
                            "// __inject_runtime_code__",
                            &self
                                .context
                                .plugin_driver
                                .runtime_plugins_code(&self.context)?,
                        )
                        .replace("// __CSS_CHUNKS_URL_MAP", &css_chunks_map_str.to_string())
                        .replace("_%main%_", module_id.generate(&self.context).as_str());

                        if !chunks_ids.is_empty() {
                            let ensures = chunks_ids
                                .into_iter()
                                .map(|id| format!("requireModule.ensure(\"{}\")", id))
                                .collect::<Vec<String>>()
                                .join(", ");

                            code.replace(
                                "// __BEFORE_ENTRY",
                                format!("Promise.all([{}]).then(()=>{{", ensures).as_str(),
                            )
                            .replace("// __AFTER_ENTRY", "});")
                        } else {
                            code
                        }
                    }
                    _ => include_str!("runtime/runtime_chunk.js")
                        .to_string()
                        .replace("_%main%_", chunk.id.generate(&self.context).as_str()),
                };
                let file_name = if matches!(chunk.chunk_type, crate::chunk::ChunkType::Entry(_, _))
                {
                    "mako_internal_runtime_entry.js"
                } else {
                    "mako_internal_runtime_chunk.js"
                };
                // TODO: handle error
                let mut js_ast = build_js_ast(file_name, content.as_str(), &self.context).unwrap();
                for stmt in &mut js_ast.ast.body {
                    // const runtime = createRuntime({ }, 'main');
                    if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(box VarDecl { decls, .. }))) = stmt
                    {
                        if decls.len() != 1 {
                            continue;
                        }
                        let decl = &mut decls[0];
                        if let Pat::Ident(BindingIdent { id, .. }) = &decl.name {
                            if id.sym.to_string() != "runtime" {
                                continue;
                            }
                        }
                        if let Some(box Expr::Call(CallExpr {
                            args,
                            callee: Callee::Expr(box Expr::Ident(ident)),
                            ..
                        })) = &mut decl.init
                        {
                            if args.len() != 2 || ident.sym.to_string() != "createRuntime" {
                                continue;
                            }
                            if let ExprOrSpread {
                                expr: box Expr::Object(ObjectLit { props, .. }),
                                ..
                            } = &mut args[0]
                            {
                                props.extend(js_stmts);
                                break;
                            }
                        }
                    }

                    // window.jsonpCallback([['main'], {}]);
                    if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                        expr:
                            box Expr::Call(CallExpr {
                                args,
                                callee:
                                    Callee::Expr(box Expr::Member(MemberExpr {
                                        obj: box Expr::Ident(ident),
                                        prop: MemberProp::Ident(ident2),
                                        ..
                                    })),
                                ..
                            }),
                        ..
                    })) = stmt
                    {
                        if args.len() != 1
                            || ident.sym.to_string() != "globalThis"
                            || ident2.sym.to_string() != "jsonpCallback"
                        {
                            continue;
                        }
                        if let ExprOrSpread {
                            expr: box Expr::Array(ArrayLit { elems, .. }),
                            ..
                        } = &mut args[0]
                        {
                            if elems.len() != 2 {
                                continue;
                            }
                            if let Some(ExprOrSpread {
                                expr: box Expr::Object(ObjectLit { props, .. }),
                                ..
                            }) = &mut elems[1]
                            {
                                props.extend(js_stmts);
                                break;
                            }
                        }
                    }
                }

                let filename = chunk.filename();
                let css_filename = get_css_chunk_filename(filename.clone());

                let mut output = vec![];
                output.push(OutputAst {
                    path: filename,
                    ast: ModuleAst::Script(js_ast),
                    chunk_id: chunk.id.id.clone(),
                    ast_module_hash: chunk.js_hash(&module_graph),
                });
                if let Some(merged_css_ast) = merged_css_ast {
                    output.push(OutputAst {
                        path: css_filename,
                        ast: ModuleAst::Css(merged_css_ast),
                        chunk_id: chunk.id.id.clone(),
                        ast_module_hash: chunk.css_hash(&module_graph),
                    });
                }
                Ok(output)
            })
            .collect::<Result<Vec<_>>>();

        match chunks_ast {
            Ok(asts) => Ok(asts.into_iter().flatten().collect::<Vec<_>>()),
            Err(e) => Err(e),
        }
    }
}

fn get_css_chunk_filename(js_chunk_filename: String) -> String {
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

fn build_ident_param(ident: Ident) -> Param {
    Param {
        span: DUMMY_SP,
        decorators: vec![],
        pat: Pat::Ident(ident.into()),
    }
}

pub fn build_fn_expr(ident: Option<Ident>, params: Vec<Param>, stmts: Vec<Stmt>) -> FnExpr {
    let func = Function {
        span: DUMMY_SP,
        params,
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
    FnExpr {
        ident,
        function: Box::new(func),
    }
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
        match ast {
            ModuleAst::Script(ast) => {
                let mut stmts = Vec::new();
                for n in ast.ast.body.iter() {
                    match n.as_stmt() {
                        None => {
                            return Err(anyhow!("Error: {:?} not a stmt in {}", n, module.id.id))
                        }
                        Some(stmt) => {
                            stmts.push(stmt.clone());
                        }
                    }
                }
                // id: function(module, exports, require) {}
                js_stmts.push(build_props(
                    module.id.generate(context).as_str(),
                    Box::new(Expr::Fn(build_fn_expr(
                        None,
                        vec![
                            build_ident_param(context.meta.script.module_ident.clone()),
                            build_ident_param(context.meta.script.exports_ident.clone()),
                            build_ident_param(context.meta.script.require_ident.clone()),
                        ],
                        stmts,
                    ))),
                ));
            }
            ModuleAst::Css(ast) => {
                // also push an empty css module to js_stmts
                // to make sure require('./xxxx.css') not throw error
                js_stmts.push(build_props(
                    module.id.generate(context).as_str(),
                    Box::new(Expr::Fn(build_fn_expr(
                        None,
                        vec![
                            build_ident_param(context.meta.script.module_ident.clone()),
                            build_ident_param(context.meta.script.exports_ident.clone()),
                            build_ident_param(context.meta.script.require_ident.clone()),
                        ],
                        vec![],
                    ))),
                ));

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
            ModuleAst::None => {}
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
