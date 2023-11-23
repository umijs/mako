use std::sync::Arc;

use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{ArrayLit, Expr, ExprOrSpread, Lit};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use super::utils::{id, member_call, member_prop, promise_all, require_ensure};
use crate::compiler::Context;
use crate::module::{generate_module_id, ModuleId};
use crate::plugins::javascript::is_dynamic_import;

pub struct DynamicImport<'a> {
    pub context: &'a Arc<Context>,
}

impl VisitMut for DynamicImport<'_> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_dynamic_import(call_expr) {
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    // note: the source is replaced !
                    let resolved_source = source.value.clone().to_string();
                    let chunk_id: ModuleId = resolved_source.clone().into();

                    let chunk_graph = &self.context.chunk_graph.read().unwrap();

                    let chunk = chunk_graph.chunk(&chunk_id);

                    let chunk_ids = match chunk {
                        Some(chunk) => {
                            let mut ids = chunk_graph
                                .sync_dependencies_chunk(&chunk.id)
                                .iter()
                                .map(|chunk_id| {
                                    generate_module_id(chunk_id.id.clone(), self.context)
                                })
                                .collect::<Vec<_>>();
                            ids.push(chunk.id.generate(self.context));
                            ids
                        }
                        // None means the original chunk has been optimized to entry chunk
                        None => vec![],
                    };

                    let to_ensure_elems = chunk_ids
                        .iter()
                        .map(|c_id| {
                            Some(ExprOrSpread {
                                spread: None,
                                expr: Box::new(require_ensure(c_id.clone())),
                            })
                        })
                        .collect::<Vec<_>>();

                    // Promise.all([ require.ensure("id") ]).then(require.bind(require, "id"))
                    // Promise.all([ require.ensure("d1"), require.ensure("id)])
                    //  .then(require.bind(require, "id"))

                    let load_promise = promise_all(ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: to_ensure_elems,
                        })),
                    });

                    let require_call = member_call(
                        Expr::Ident(id("__mako_require__")),
                        member_prop("bind"),
                        vec![
                            ExprOrSpread {
                                spread: None,
                                expr: Box::new(Expr::Ident(id("__mako_require__"))),
                            },
                            ExprOrSpread {
                                spread: None,
                                expr: Box::new(Expr::Lit(Lit::Str(resolved_source.into()))),
                            },
                        ],
                    );

                    *expr = member_call(
                        load_promise,
                        member_prop("then"),
                        vec![ExprOrSpread {
                            spread: None,
                            expr: Box::new(require_call),
                        }],
                    );
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

// utils fn

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::swc_common::{Globals, GLOBALS};
    use mako_core::swc_ecma_visit::VisitMutWith;

    use super::DynamicImport;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::chunk::{Chunk, ChunkType};
    use crate::compiler::Context;

    #[test]
    fn test_dyanmic_import() {
        let code = r#"
import("./foo");
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
Promise.all([
    __mako_require__.ensure("./foo")
]).then(__mako_require__.bind(__mako_require__, "./foo"));

//# sourceMappingURL=index.js.map
            "#
            .trim()
        );
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context: Arc<Context> = Arc::new(Default::default());

        let mut chunk = Chunk::new("./foo".to_string().into(), ChunkType::Async);
        chunk.add_module("./foo".to_string().into());

        context.chunk_graph.write().unwrap().add_chunk(chunk);

        let mut ast = build_js_ast(path, origin, &context).unwrap();

        let globals = Globals::default();
        GLOBALS.set(&globals, || {
            let mut dyanmic_import = DynamicImport { context: &context };
            ast.ast.visit_mut_with(&mut dyanmic_import);
        });

        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
