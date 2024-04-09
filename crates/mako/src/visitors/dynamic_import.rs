use std::sync::Arc;

use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{ArrayLit, Expr, ExprOrSpread, Lit};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::ast_2::utils::{
    id, is_dynamic_import, member_call, member_prop, promise_all, require_ensure,
};
use crate::chunk::ChunkId;
use crate::compiler::Context;

pub struct DynamicImport {
    pub context: Arc<Context>,
}

impl VisitMut for DynamicImport {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_dynamic_import(call_expr) {
                if call_expr.args.is_empty() {
                    return;
                }
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    // note: the source is replaced!
                    let resolved_source = source.value.clone().to_string();
                    let chunk_ids = {
                        let chunk_id: ChunkId = resolved_source.clone().into();
                        let chunk_graph = &self.context.chunk_graph.read().unwrap();
                        let chunk = chunk_graph.chunk(&chunk_id);
                        let chunk_ids = match chunk {
                            Some(chunk) => {
                                [
                                    chunk_graph.sync_dependencies_chunk(&chunk.id),
                                    vec![chunk.id.clone()],
                                ]
                                .concat()
                                .iter()
                                .filter_map(|chunk_id| {
                                    // skip empty chunk because it will not be generated
                                    if chunk_graph
                                        .chunk(chunk_id)
                                        .is_some_and(|c| !c.modules.is_empty())
                                    {
                                        Some(chunk_id.id.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                            }
                            // None means the original chunk has been optimized to entry chunk
                            None => vec![],
                        };
                        chunk_ids
                    };

                    // build new expr
                    // e.g.
                    // Promise.all([ require.ensure("id") ]).then(require.bind(require, "id"))
                    // Promise.all([ require.ensure("d1"), require.ensure("id)]).then(require.bind(require, "id"))
                    *expr = {
                        let to_ensure_elems = chunk_ids
                            .iter()
                            .map(|c_id| {
                                Some(ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(require_ensure(c_id.clone())),
                                })
                            })
                            .collect::<Vec<_>>();
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
                        member_call(
                            load_promise,
                            member_prop("then"),
                            vec![ExprOrSpread {
                                spread: None,
                                expr: Box::new(require_call),
                            }],
                        )
                    };
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_visit::VisitMutWith;

    use super::DynamicImport;
    use crate::ast_2::tests::TestUtils;

    // TODO: more precise test, it now has not chunks info
    #[test]
    fn test_dynamic_import() {
        assert_eq!(
            run(r#"import("./foo");"#),
            r#"Promise.all([]).then(__mako_require__.bind(__mako_require__, "./foo"));"#
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code.to_string());
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DynamicImport {
                context: test_utils.context.clone(),
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
