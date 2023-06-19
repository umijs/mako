use std::collections::HashSet;
use std::vec;

use anyhow::Result;
use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, BindingIdent, BlockStmt, CallExpr, Callee, Decl, Expr, ExprOrSpread, ExprStmt,
    FnExpr, Function, Ident, KeyValueProp, MemberExpr, MemberProp, Module, ModuleItem, ObjectLit,
    Param, Pat, Prop, PropOrSpread, Stmt, Str, VarDecl,
};
use tracing::info;

use crate::ast::build_js_ast;
use crate::compiler::Compiler;
use crate::module::{ModuleAst, ModuleId};

pub struct OutputFile {
    pub path: String,
    pub content: String,
    pub sourcemap: String,
    pub js_ast: Module,
}

impl Compiler {
    pub fn generate_chunks(&self) -> Result<Vec<OutputFile>> {
        info!("generate chunks");
        let module_graph = self.context.module_graph.read().unwrap();
        let mut chunk_graph = self.context.chunk_graph.write().unwrap();

        let public_path = self.context.config.public_path.clone();
        let public_path = if public_path == "runtime" {
            "globalThis.publicPath".to_string()
        } else {
            format!("\"{}\"", public_path)
        };
        let mut chunks = chunk_graph.chunks_mut();
        // TODO: remove this
        let chunks_map_str: Vec<String> = chunks
            .iter()
            .map(|chunk| {
                format!(
                    "chunksIdToUrlMap[\"{}\"] = `${{{}}}{}`;",
                    chunk.id.id,
                    public_path,
                    chunk.filename()
                )
            })
            .collect();
        let chunks_map_str = format!(
            "const chunksIdToUrlMap = {{}};\n{}",
            chunks_map_str.join("\n")
        );

        let output_files = chunks
            // TODO:
            // 由于任务划分不科学，rayon + par_iter 没啥效果
            .iter_mut()
            .map(|chunk| {
                // build stmts
                let module_ids = chunk.get_modules();
                let js_stmts = modules_to_js_stmts(module_ids, &module_graph);

                // build js ast
                let mut content = if matches!(chunk.chunk_type, crate::chunk::ChunkType::Entry) {
                    format!(
                        "{}\n{}",
                        chunks_map_str,
                        include_str!("runtime/runtime_entry.js")
                    )
                } else {
                    include_str!("runtime/runtime_chunk.js").to_string()
                };
                content = content.replace("main", chunk.id.id.as_str());
                let file_name = if matches!(chunk.chunk_type, crate::chunk::ChunkType::Entry) {
                    "mako_internal_runtime_entry.js"
                } else {
                    "mako_internal_runtime_chunk.js"
                };
                // TODO: handle error
                let mut js_ast = build_js_ast(file_name, content.as_str(), &self.context).unwrap();
                for stmt in &mut js_ast.body {
                    // const runtime = createRuntime({}, 'main');
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

                // build css ast
                // TODO
                // 暂时无需处理

                // minify
                // if matches!(self.context.config.mode, Mode::Production) {
                //     js_ast = minify_js(js_ast, &self.context.meta.script.cm);
                // }

                let filename = chunk.filename();
                // let (js_code, js_sourcemap) = js_ast_to_code(&js_ast, &self.context, &filename);

                // chunk.cache_content(js_code.clone(), js_sourcemap.clone());

                OutputFile {
                    path: filename,
                    content: "".to_string(),
                    sourcemap: "".to_string(),
                    js_ast,
                    // filename: chunk.filename(),
                }
            })
            .collect();
        Ok(output_files)
    }
}

fn build_ident_param(ident: &str) -> Param {
    Param {
        span: DUMMY_SP,
        decorators: vec![],
        pat: Pat::Ident(BindingIdent {
            id: Ident::new(ident.into(), DUMMY_SP),
            type_ann: None,
        }),
    }
}

fn build_fn_expr(ident: Option<Ident>, params: Vec<Param>, stmts: Vec<Stmt>) -> FnExpr {
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

fn build_props(key_str: &str, value: Box<Expr>) -> PropOrSpread {
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
    module_ids: &HashSet<ModuleId>,
    module_graph: &std::sync::RwLockReadGuard<crate::module_graph::ModuleGraph>,
) -> Vec<PropOrSpread> {
    let mut js_stmts = vec![];
    let mut module_ids: Vec<_> = module_ids.iter().collect();
    module_ids.sort_by_key(|module_id| module_id.id.to_string());
    module_ids.iter().for_each(|module_id| {
        let module = module_graph.get_module(module_id).unwrap();
        let ast = module.info.as_ref().unwrap();
        let ast = &ast.ast;
        match ast {
            ModuleAst::Script(ast) => {
                // id: function(module, exports, require) {}
                js_stmts.push(build_props(
                    module.id.id.as_str(),
                    Box::new(Expr::Fn(build_fn_expr(
                        None,
                        vec![
                            build_ident_param("module"),
                            build_ident_param("exports"),
                            build_ident_param("require"),
                        ],
                        ast.body
                            .iter()
                            .map(|stmt| stmt.as_stmt().unwrap().clone())
                            .collect(),
                    ))),
                ));
            }
            ModuleAst::Css(_ast) => {
                // TODO:
                // 目前 transform_all 之后，css 的 ast 会变成 js 的 ast，所以这里不需要处理
                // 之后如果要支持提取独立的 css 文件，会需要在这里进行处理
            }
            ModuleAst::None => {}
        }
    });
    js_stmts
}
