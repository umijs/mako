use anyhow::Result;
use indexmap::IndexSet;
use swc_core::ecma::ast::{
    CallExpr, Expr, ExprOrSpread, ExprStmt, KeyValueProp, ModuleItem, ObjectLit, Prop,
    PropOrSpread, Stmt,
};

use crate::ast::js_ast::{JSAstGenerated, JsAst};
use crate::compiler::Compiler;
use crate::generate::chunk::Chunk;
use crate::generate::generate_chunks::modules_to_js_stmts;
use crate::module::ModuleId;

impl Compiler {
    pub fn generate_hmr_chunk(
        &self,
        chunk: &Chunk,
        filename: &str,
        module_ids: &IndexSet<ModuleId>,
        current_hash: u64,
    ) -> Result<(String, String)> {
        let module_graph = &self.context.module_graph.read().unwrap();
        let (js_stmts, _) = modules_to_js_stmts(module_ids, module_graph, &self.context).unwrap();
        let content = include_str!("../runtime/runtime_hmr.js").to_string();

        let runtime_code_snippets = [
            format!("runtime._h='{}';", current_hash),
            self.context
                .plugin_driver
                .hmr_runtime_update_code(&self.context)?,
        ];

        let content = content
            .replace("__CHUNK_ID__", &chunk.id.id)
            .replace("__runtime_code__", &runtime_code_snippets.join("\n"));

        let mut js_ast = JsAst::build(filename, content.as_str(), self.context.clone())
            /* safe */
            .unwrap();

        for stmt in &mut js_ast.ast.body {
            if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                expr: box Expr::Call(CallExpr { args, .. }),
                ..
            })) = stmt
            {
                if let ExprOrSpread {
                    expr: box Expr::Object(ObjectLit { props, .. }),
                    ..
                } = &mut args[1]
                {
                    if props.len() != 1 {
                        panic!("hmr runtime should only have one modules property");
                    }
                    if let PropOrSpread::Prop(box Prop::KeyValue(KeyValueProp {
                        value: box Expr::Object(ObjectLit { props, .. }),
                        ..
                    })) = &mut props[0]
                    {
                        props.extend(js_stmts);
                        break;
                    }
                }
            }
        }

        let JSAstGenerated { code, sourcemap } = js_ast.generate(self.context.clone()).unwrap();
        Ok((code, sourcemap))
    }
}
