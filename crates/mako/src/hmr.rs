use std::collections::HashSet;

use swc_ecma_ast::{
    CallExpr, Expr, ExprOrSpread, ExprStmt, KeyValueProp, ModuleItem, ObjectLit, Prop,
    PropOrSpread, Stmt,
};

use crate::{
    ast::{build_js_ast, js_ast_to_code},
    chunk::Chunk,
    compiler::Compiler,
    generate_chunks::modules_to_js_stmts,
    module::ModuleId,
};

impl Compiler {
    pub fn generate_hmr_chunk(
        &self,
        chunk: &Chunk,
        module_ids: &HashSet<ModuleId>,
    ) -> (String, String) {
        let module_graph = &self.context.module_graph.read().unwrap();
        let js_stmts = modules_to_js_stmts(module_ids, module_graph);
        let mut content = include_str!("runtime/runtime_hmr.js").to_string();
        content = content.replace("__CHUNK_ID__", &chunk.id.id);
        let filename = &chunk.filename();
        // TODO: handle error
        let mut js_ast = build_js_ast(filename, content.as_str(), &self.context).unwrap();

        for stmt in &mut js_ast.body {
            if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                expr: box Expr::Call(CallExpr { args, .. }),
                ..
            })) = stmt
            {
                if args.len() != 1 {
                    panic!("hmr runtime should only have one argument");
                }
                if let ExprOrSpread {
                    expr: box Expr::Object(ObjectLit { props, .. }),
                    ..
                } = &mut args[0]
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

        let (js_code, js_sourcemap) = js_ast_to_code(&js_ast, &self.context, filename);
        (js_code, js_sourcemap)
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::Compiler;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_generate_hmr_chunk() {
        let compiler = create_compiler("test/build/normal");
        compiler.build();
        compiler.group_chunk();
        let chunk_graph = &compiler.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();
        let chunk = chunks[0];
        let module_ids = chunk.get_modules();
        let (js_code, _js_sourcemap) = compiler.generate_hmr_chunk(chunk, module_ids);
        let js_code = js_code.replace(
            compiler.context.root.to_string_lossy().to_string().as_str(),
            "",
        );
        println!("{}", js_code);
        assert_eq!(
            js_code.trim(),
            r#"
modulesRegistry['/index.ts'].hot.apply({
    modules: {
        "/bar_1.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("/foo.ts");
        },
        "/bar_2.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("/foo.ts");
        },
        "/foo.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            Object.defineProperty(exports, "default", {
                enumerable: true,
                get: function() {
                    return _default;
                }
            });
            var _default = 1;
        },
        "/index.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("/bar_1.ts");
            require("/bar_2.ts");
        }
    }
});

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn create_compiler(base: &str) -> Compiler {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Default::default();
        Compiler::new(config, root)
    }
}
