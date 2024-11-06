use std::sync::Arc;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{
    ArrayLit, Expr, ExprOrSpread, Ident, Lit, MemberExpr, Module, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{
    member_expr, private_ident, quote_ident, quote_str, ExprFactory, IsDirective,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use super::dep_replacer::{miss_throw_stmt, ResolvedReplaceInfo};
use crate::ast::utils::{is_dynamic_import, promise_all, require_ensure};
use crate::ast::DUMMY_CTXT;
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
                .position(|module_item| !module_item.directive_continue())
                .unwrap();

            let interop_replace = self
                .dep_to_replace
                .resolved
                .get("@swc/helpers/_/_interop_require_wildcard")
                .unwrap();

            let require_interop = quote_ident!("__mako_require__").as_call(
                DUMMY_SP,
                vec![quote_str!(interop_replace.to_replace_source.clone()).as_arg()],
            );

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
                    if self
                        .dep_to_replace
                        .missing
                        .contains_key(source.value.as_ref())
                    {
                        call_expr.args[0] = ExprOrSpread {
                            spread: None,
                            expr: Box::new(miss_throw_stmt(source.value.as_ref())),
                        };
                        return;
                    }

                    let resolved_info = self.dep_to_replace.resolved.get(source.value.as_ref());

                    // e.g.
                    // import(/* webpackIgnore: true */ "foo")
                    // will be ignored
                    if resolved_info.is_none() {
                        return;
                    }

                    let resolved_info = resolved_info
                        // If the identifier is not in dep_to_replace.missing,
                        // it must be resolved, so unwrap is safe here.
                        .unwrap();

                    self.changed = true;

                    let generated_module_id = resolved_info.to_replace_source.clone();
                    *expr = {
                        // let load_promise = self.make_load_promise(&chunk_ids);

                        let load_promise = if self.context.args.watch
                            && self.context.config.experimental.central_ensure
                        {
                            self.central_ensure(&generated_module_id)
                        } else {
                            self.inline_ensure(resolved_info, &self.context)
                        };

                        let lazy_require_call =
                            member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_require__.bind).as_call(
                                DUMMY_SP,
                                vec![
                                    quote_ident!("__mako_require__").as_arg(),
                                    quote_str!(generated_module_id).as_arg(),
                                ],
                            );
                        let dr_call = member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_require__.dr)
                            .as_call(
                                DUMMY_SP,
                                vec![self.interop.clone().as_arg(), lazy_require_call.as_arg()],
                            );

                        member_expr!(@EXT, DUMMY_SP, load_promise.into(), then)
                            .as_call(call_expr.span, vec![dr_call.as_arg()])
                    };
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

impl DynamicImport<'_> {
    // require.ensure2("id").then(require.bind(require,"id"))
    fn central_ensure(&self, module_id: &str) -> Expr {
        member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_require__.ensure2)
            .as_call(DUMMY_SP, vec![quote_str!(module_id).as_arg()])
    }

    // build the Promise.all([...]) part
    // Promise.all([ require.ensure("id") ]).then(require.bind(require, "id"))
    // Promise.all([ require.ensure("d1"), require.ensure("id)]).then(require.bind(require, "id"))
    fn inline_ensure(&self, replace_info: &ResolvedReplaceInfo, context: &Arc<Context>) -> Expr {
        let chunk_graph = context.chunk_graph.read().unwrap();

        let init_chunk_id: ChunkId = replace_info.chunk_id.as_ref().unwrap().clone().into();
        let chunk_ids = {
            let chunk = chunk_graph.chunk(&init_chunk_id);
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

        let to_ensure_elems = chunk_ids
            .iter()
            .map(|c_id| {
                Some(ExprOrSpread {
                    spread: None,
                    expr: Box::new(require_ensure(c_id.clone())),
                })
            })
            .collect::<Vec<_>>();
        promise_all(ExprOrSpread {
            spread: None,
            expr: ArrayLit {
                span: DUMMY_SP,
                elems: to_ensure_elems,
            }
            .into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::DynamicImport;
    use crate::ast::tests::TestUtils;
    use crate::generate::chunk::{Chunk, ChunkType};
    use crate::visitors::dep_replacer::{DependenciesToReplace, ResolvedReplaceInfo};

    // TODO: add nested chunk test
    #[test]
    fn test_dynamic_import() {
        assert_eq!(
            run(r#"import("foo");"#),
            r#"
var interop = __mako_require__("hashed_helper")._;
Promise.all([
    __mako_require__.ensure("foo")
]).then(__mako_require__.dr(interop, __mako_require__.bind(__mako_require__, "foo")));
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
                "@swc/helpers/_/_interop_require_wildcard".to_string() => ResolvedReplaceInfo {
                    chunk_id: None,
                    to_replace_source: "hashed_helper".to_string(),
                    resolved_module_id:"dummy".into()
                },
                "foo".to_string() => ResolvedReplaceInfo {
                    chunk_id: Some("foo".into()),
                    to_replace_source: "foo".into(),
                    resolved_module_id: "foo".into()
                }
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
