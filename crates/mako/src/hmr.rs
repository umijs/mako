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

#[cfg(test)]
mod tests {
    use mako_core::tokio;

    use crate::assert_debug_snapshot;
    use crate::compiler::{Args, Compiler};
    use crate::config::Config;
    use crate::transform_in_generate::transform_modules;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_generate_hmr_chunk() {
        let compiler = create_compiler("test/dev/normal");

        compiler.build().unwrap();
        compiler.group_chunk();
        let chunk_graph = &compiler.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();
        let chunk = chunks[0];
        let module_ids = chunk.get_modules();
        transform_modules(module_ids.iter().cloned().collect(), &compiler.context).unwrap();
        let (js_code, _js_sourcemap) = compiler
            .generate_hmr_chunk(chunk, "index.js", module_ids, 42)
            .unwrap();
        let js_code = js_code.replace(
            compiler.context.root.to_string_lossy().to_string().as_str(),
            "",
        );
        println!("{}", js_code);

        assert_debug_snapshot!(js_code.trim());
    }

    fn create_compiler(base: &str) -> Compiler {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        Compiler::new(config, root, Args { watch: true }).unwrap()
    }
}
