use std::sync::Arc;

use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{ArrayLit, Expr, ExprOrSpread, Lit, MemberExpr};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_core::ecma::ast::{Ident, Module, Stmt, VarDeclKind};
use swc_core::ecma::utils::{
    member_expr, private_ident, quote_ident, quote_str, ExprFactory, IsDirective,
};

use crate::ast::utils::{is_dynamic_import, member_call, member_prop, promise_all, require_ensure};
use crate::compiler::Context;
use crate::generate::chunk::ChunkId;
use crate::visitors::dep_replacer::DependenciesToReplace;

pub struct DynamicImport<'a> {
    pub context: Arc<Context>,
    interop: Ident,
    changed: bool,
    dep_to_replace: &'a DependenciesToReplace,
}

impl<'a> DynamicImport<'a> {
    pub fn new(context: Arc<Context>, dep_map: &'a DependenciesToReplace) -> Self {
        let interop = private_ident!("interop");

        Self {
            context,
            interop,
            changed: false,
            dep_to_replace: dep_map,
        }
    }
}

impl<'a> VisitMut for DynamicImport<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        n.visit_mut_children_with(self);

        if self.changed {
            let insert_at = n
                .body
                .iter()
                .position(|module_item| {
                    !module_item
                        .as_stmt()
                        .map_or(false, |stmt| stmt.is_directive())
                })
                .unwrap();

            let (id, _) = self
                .dep_to_replace
                .resolved
                .get("@swc/helpers/_/_interop_require_wildcard")
                .unwrap();

            let require_interop = quote_ident!("__mako_require__")
                .as_call(DUMMY_SP, vec![quote_str!(id.clone()).as_arg()]);

            let stmt: Stmt = Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: require_interop.into(),
                prop: quote_ident!("_").into(),
            })
            .into_var_decl(VarDeclKind::Var, self.interop.clone().into())
            .into();

            n.body.insert(insert_at, stmt.into());
        }
    }

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

                    self.changed = true;
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

                        let require_call = member_expr!(DUMMY_SP, __mako_require__.dr).as_call(
                            DUMMY_SP,
                            vec![
                                self.interop.clone().as_arg(),
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
                                expr: require_call.into(),
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
    use std::collections::HashMap;

    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_visit::VisitMutWith;

    use super::DynamicImport;
    use crate::ast::tests::TestUtils;
    use crate::generate::chunk::{Chunk, ChunkType};
    use crate::visitors::dep_replacer::DependenciesToReplace;

    // TODO: add nested chunk test
    #[test]
    fn test_dynamic_import() {
        assert_eq!(
            run(r#"import("foo");"#),
            r#"
var interop = __mako_require__("hashed_helper")._;
Promise.all([
    __mako_require__.ensure("foo")
]).then(__mako_require__.dr(interop, "foo"));
            "#
            .trim()
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        {
            let mut foo = Chunk::new("foo".to_string().into(), ChunkType::Async);
            foo.add_module("foo".to_string().into());
            let mut cg = test_utils.context.chunk_graph.write().unwrap();
            cg.add_chunk(foo);
        }
        let ast = test_utils.ast.js_mut();

        let dep_to_replace = DependenciesToReplace {
            resolved: maplit::hashmap! {
                "@swc/helpers/_/_interop_require_wildcard".to_string() =>
                ("hashed_helper".to_string(), "dummy".into())
            },
            missing: HashMap::new(),
        };

        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut visitor = DynamicImport::new(test_utils.context.clone(), &dep_to_replace);
            ast.ast.visit_mut_with(&mut visitor);
        });
        let code = test_utils.js_ast_to_code();
        println!("{}", code);
        code
    }
}
