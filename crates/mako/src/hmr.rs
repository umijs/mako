use std::collections::HashSet;

use anyhow::Result;
use swc_ecma_ast::{
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
        module_ids: &HashSet<ModuleId>,
        current_hash: u64,
    ) -> Result<(String, String)> {
        let module_graph = &self.context.module_graph.read().unwrap();
        let (js_stmts, _) = modules_to_js_stmts(module_ids, module_graph, &self.context);
        let mut content = include_str!("runtime/runtime_hmr.js").to_string();
        content = content
            .replace("__CHUNK_ID__", &chunk.id.generate(&self.context))
            .replace(
                "__runtime_code__",
                &format!("runtime._h='{}';", current_hash),
            );
        let filename = &chunk.filename();
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

    use crate::compiler::Compiler;
    use crate::config::Config;
    use crate::transform_in_generate::transform_modules;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_generate_hmr_chunk() {
        let compiler = create_compiler("test/dev/normal");

        compiler.build();
        compiler.group_chunk();
        let chunk_graph = &compiler.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();
        let chunk = chunks[0];
        let module_ids = chunk.get_modules();
        transform_modules(module_ids.iter().cloned().collect(), &compiler.context).unwrap();
        let (js_code, _js_sourcemap) = compiler.generate_hmr_chunk(chunk, module_ids, 42).unwrap();
        let js_code = js_code.replace(
            compiler.context.root.to_string_lossy().to_string().as_str(),
            "",
        );
        println!("{}", js_code);
        assert_eq!(
            js_code.trim(),
            r#"
globalThis.makoModuleHotUpdate('./index.ts', {
    modules: {
        "./bar_1.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("./foo.ts");
        },
        "./bar_2.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("./foo.ts");
        },
        "./foo.ts": function(module, exports, require) {
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
        "./index.ts": function(module, exports, require) {
            const RefreshRuntime = require("../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/runtime.js");
            RefreshRuntime.injectIntoGlobalHook(window);
            window.$RefreshReg$ = ()=>{};
            window.$RefreshSig$ = ()=>(type)=>type;
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/runtime.js");
            require("./bar_1.ts");
            require("./bar_2.ts");
            require("./hoo");
            (function() {
                const socket = new WebSocket('ws://127.0.0.1:3000/__/hmr-ws');
                let latestHash = '';
                let updating = false;
                function runHotUpdate() {
                    if (latestHash !== require.currentHash()) {
                        updating = true;
                        return module.hot.check().then(()=>{
                            updating = false;
                            return runHotUpdate();
                        }).catch((e)=>{
                            console.error('[HMR] HMR check failed', e);
                        });
                    } else {
                        return Promise.resolve();
                    }
                }
                socket.addEventListener('message', (rawMessage)=>{
                    console.log(rawMessage);
                    const msg = JSON.parse(rawMessage.data);
                    latestHash = msg.hash;
                    if (!updating) {
                        runHotUpdate();
                    }
                });
            })();
        },
        "./hoo": function(module, exports, require) {
            "use strict";
            module.exports = hoo;
        }
    }
}, function(runtime) {
    runtime._h = '42';
    ;
});

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn create_compiler(base: &str) -> Compiler {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        Compiler::new(config, root)
    }
}
