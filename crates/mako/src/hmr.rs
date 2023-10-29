use mako_core::anyhow::Result;
use mako_core::indexmap::IndexSet;
use mako_core::swc_ecma_ast::{
    CallExpr, Expr, ExprOrSpread, ExprStmt, KeyValueProp, ModuleItem, ObjectLit, Prop,
    PropOrSpread, Stmt,
};

use crate::ast::{build_js_ast, js_ast_to_code};
use crate::chunk::Chunk;
use crate::compiler::Compiler;
use crate::generate_chunks::modules_to_js_stmts;
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
        let mut content = include_str!("runtime/runtime_hmr.js").to_string();
        content = content
            .replace("__CHUNK_ID__", &chunk.id.generate(&self.context))
            .replace(
                "__runtime_code__",
                &format!("runtime._h='{}';", current_hash),
            );
        // TODO: handle error
        let mut js_ast = build_js_ast(filename, content.as_str(), &self.context)
            .unwrap()
            .ast;

        for stmt in &mut js_ast.body {
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

        let (js_code, js_sourcemap) = js_ast_to_code(&js_ast, &self.context, filename).unwrap();
        Ok((js_code, js_sourcemap))
    }
}
