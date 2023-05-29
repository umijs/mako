use rayon::prelude::*;
use swc_ecma_ast::{Callee, Expr, ExprOrSpread, ExprStmt, ModuleItem, Stmt};
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

        let chunks = chunk_graph.get_chunks();
        let output_files = chunks
            // TODO:
            // 由于任务划分不科学，rayon + par_iter 没啥效果
            .par_iter()
            .map(|chunk| {
                // build stmts
                let mut js_stmts = vec![];
                let modules = chunk.get_modules();
                modules.iter().for_each(|module_id| {
                    let module = module_graph.get_module(module_id).unwrap();
                    let ast = module.info.as_ref().unwrap();
                    let ast = &ast.ast;
                    match ast {
                        ModuleAst::Script(ast) => js_stmts
                            .extend(ast.body.iter().map(|stmt| stmt.as_stmt().unwrap().clone())),
                        ModuleAst::Css(_ast) => {
                            // TODO:
                            // 目前 transform_all 之后，css 的 ast 会变成 js 的 ast，所以这里不需要处理
                            // 之后如果要支持提取独立的 css 文件，会需要在这里进行处理
                        }
                        ModuleAst::None => {}
                    }
                });

                // build js ast
                // TODO: support chunk, 目前只支持 entry
                let mut content = include_str!("runtime/runtime_entry.js").to_string();
                content = content.replace("main", chunk.id.id.as_str());
                let mut js_ast = build_js_ast("index.js", content.as_str(), &self.context);
                for stmt in &mut js_ast.body {
                    if let ModuleItem::Stmt(Stmt::Expr(expr)) = stmt {
                        if let ExprStmt {
                            expr: box Expr::Call(call_expr),
                            ..
                        } = expr
                        {
                            let is_register_modules =
                                if let Callee::Expr(box Expr::Ident(ident)) = &call_expr.callee {
                                    ident.sym.to_string() == "registerModules"
                                } else {
                                    false
                                };
                            if !is_register_modules {
                                continue;
                            }

                            if let ExprOrSpread {
                                expr: box Expr::Fn(func),
                                ..
                            } = &mut call_expr.args[0]
                            {
                                func.function.body.as_mut().unwrap().stmts.extend(js_stmts);
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
