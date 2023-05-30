use std::vec;

use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, BindingIdent, BlockStmt, CallExpr, Callee, Decl, Expr, ExprOrSpread, ExprStmt,
    FnExpr, Function, Ident, KeyValueProp, MemberExpr, MemberProp, ModuleItem, ObjectLit, Param,
    Pat, Prop, PropOrSpread, Stmt, Str, VarDecl,
};
use tracing::info;

use crate::{
    ast::{build_js_ast, js_ast_to_code},
    compiler::Compiler,
    config::Mode,
    minify::minify_js,
    module::ModuleAst,
};

pub struct OutputFile {
    pub path: String,
    pub content: String,
    pub sourcemap: String,
}

impl Compiler {
    pub fn generate_chunks(&self) -> Vec<OutputFile> {
        info!("generate chunks");
        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let mut public_path = self.context.config.public_path.clone();
        public_path = if public_path == "runtime" {
            "globalThis.publicPath".to_string()
        } else {
            format!("\"{}\"", public_path)
        };
        let chunks = chunk_graph.get_chunks();
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
            .iter()
            .map(|chunk| {
                // build stmts
                let mut js_stmts = vec![];
                let modules = chunk.get_modules();
                modules.iter().for_each(|module_id| {
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
                let mut js_ast = build_js_ast(file_name, content.as_str(), &self.context);
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
                                props.extend(js_stmts.clone());
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
                                props.extend(js_stmts.clone());
                                break;
                            }
                        }
                    }
                }

                // build css ast
                // TODO
                // 暂时无需处理

                // minify
                if matches!(self.context.config.mode, Mode::Production) {
                    js_ast = minify_js(js_ast, &self.context.meta.script.cm);
                }

                let filename = chunk.filename();
                let (js_code, js_sourcemap) = js_ast_to_code(&js_ast, &self.context, &filename);
                OutputFile {
                    path: filename,
                    content: js_code,
                    sourcemap: js_sourcemap,
                }
            })
            .collect();
        output_files
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
