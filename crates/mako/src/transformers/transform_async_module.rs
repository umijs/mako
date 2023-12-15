use std::sync::Arc;

use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{
    ArrayLit, ArrowExpr, AssignExpr, AssignOp, AwaitExpr, BindingIdent, BlockStmt, BlockStmtOrExpr,
    CallExpr, Callee, CondExpr, Decl, Expr, ExprOrSpread, ExprStmt, Ident, Lit, MemberExpr,
    MemberProp, ModuleItem, ParenExpr, Pat, PatOrExpr, Stmt, Str, VarDecl, VarDeclKind,
    VarDeclarator,
};
use mako_core::swc_ecma_visit::VisitMut;

use crate::compiler::Context;
use crate::module::Dependency;
use crate::plugins::javascript::is_commonjs_require;

const ASYNC_DEPS_IDENT: &str = "__mako_async_dependencies__";
const ASYNC_IMPORTED_MODULE: &str = "_async__mako_imported_module_";

pub struct AsyncModule<'a> {
    pub async_deps: &'a Vec<Dependency>,
    pub async_deps_idents: Vec<BindingIdent>,
    pub last_dep_pos: usize,
    pub top_level_await: bool,
    pub context: &'a Arc<Context>,
    pub unresolved_mark: Mark,
}

impl VisitMut for AsyncModule<'_> {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        // Collect the idents of all async deps, while recording the position of the last import statement
        for (i, module_item) in module_items.iter_mut().enumerate() {
            if let ModuleItem::Stmt(stmt) = module_item {
                match stmt {
                    // `require('./async');` => `var _async__mako_imported_module_n__ = require('./async');`
                    Stmt::Expr(expr_stmt) => {
                        if let Expr::Call(call_expr) = &*expr_stmt.expr {
                            if is_commonjs_require(call_expr, &self.unresolved_mark) {
                                if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                    &*call_expr.args[0].expr
                                {
                                    let source = value.to_string();
                                    if self.async_deps.iter().any(|dep| dep.source == source) {
                                        let ident_name: BindingIdent = Ident::new(
                                            format!("{}{}__", ASYNC_IMPORTED_MODULE, i).into(),
                                            DUMMY_SP,
                                        )
                                        .into();
                                        *stmt = Stmt::Decl(Decl::Var(Box::new(VarDecl {
                                            span: DUMMY_SP,
                                            kind: VarDeclKind::Var,
                                            declare: false,
                                            decls: vec![VarDeclarator {
                                                span: DUMMY_SP,
                                                name: Pat::Ident(ident_name.clone()),
                                                init: Some(Box::new(*expr_stmt.expr.clone())),
                                                definite: false,
                                            }],
                                        })));
                                        self.async_deps_idents.push(ident_name.clone());
                                        self.last_dep_pos = i;
                                    }
                                }
                            }
                        }
                    }

                    // `var async = require('./async');`
                    Stmt::Decl(Decl::Var(var_decl)) => {
                        for decl in &var_decl.decls {
                            if let Some(box Expr::Call(call_expr)) = &decl.init {
                                if is_commonjs_require(call_expr, &self.unresolved_mark) {
                                    if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                        &*call_expr.args[0].expr
                                    {
                                        let source = value.to_string();
                                        if self.async_deps.iter().any(|dep| dep.source == source) {
                                            // filter the async deps
                                            if let Pat::Ident(binding_ident) = &decl.name {
                                                self.async_deps_idents.push(binding_ident.clone());
                                                self.last_dep_pos = i;
                                            }
                                        }
                                    }
                                } else if let Callee::Expr(box Expr::Member(MemberExpr {
                                    obj,
                                    prop,
                                    ..
                                })) = &call_expr.callee
                                {
                                    if let Some(Ident { sym: obj_sym, .. }) = obj.clone().ident() {
                                        if &obj_sym == "_interop_require_default"
                                            || &obj_sym == "_interop_require_wildcard"
                                        {
                                            if let Some(Ident { sym, .. }) = prop.clone().ident() {
                                                if &sym == "_" {
                                                    if let Expr::Call(call_expr) =
                                                        &*call_expr.args[0].expr
                                                    {
                                                        if is_commonjs_require(
                                                            call_expr,
                                                            &self.unresolved_mark,
                                                        ) {
                                                            if let Expr::Lit(Lit::Str(Str {
                                                                value,
                                                                ..
                                                            })) = &*call_expr.args[0].expr
                                                            {
                                                                let source = value.to_string();
                                                                if self
                                                                    .async_deps
                                                                    .iter()
                                                                    .any(|dep| dep.source == source)
                                                                {
                                                                    // filter the async deps
                                                                    if let Pat::Ident(
                                                                        binding_ident,
                                                                    ) = &decl.name
                                                                    {
                                                                        let binding_ident_default = BindingIdent {
                                                                            id: Ident {
                                                                                sym: format!("{}.default", binding_ident.id.sym).into(),
                                                                                ..binding_ident.id.clone()
                                                                            },
                                                                            ..binding_ident.clone()
                                                                        };

                                                                        if &obj_sym == "_interop_require_default" {
                                                                            // ex. _react.default
                                                                            self.async_deps_idents.push(binding_ident_default.clone());
                                                                        } else if &obj_sym == "_interop_require_wildcard" {
                                                                            // ex. _react
                                                                            self.async_deps_idents.push(binding_ident.clone());
                                                                            // why also push the default import?
                                                                            // adapt both default import and wildcard import for the same module
                                                                            self.async_deps_idents.push(binding_ident_default.clone());
                                                                        } else {
                                                                            unreachable!();
                                                                        }

                                                                        self.last_dep_pos = i;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if !self.async_deps_idents.is_empty() {
            // Insert code after the last import statement: `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
            module_items.insert(
                self.last_dep_pos + 1,
                ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(VarDecl {
                    span: DUMMY_SP,
                    kind: VarDeclKind::Var,
                    declare: false,
                    decls: vec![VarDeclarator {
                        span: DUMMY_SP,
                        name: Pat::Ident(BindingIdent {
                            id: Ident {
                                span: DUMMY_SP,
                                sym: ASYNC_DEPS_IDENT.into(),
                                optional: false,
                            },
                            type_ann: None,
                        }),
                        init: Some(Box::new(Expr::Call(CallExpr {
                            span: DUMMY_SP,
                            callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                                span: DUMMY_SP,
                                sym: "handleAsyncDeps".into(),
                                optional: false,
                            }))),
                            args: vec![ExprOrSpread {
                                spread: None,
                                expr: Box::new(Expr::Array(ArrayLit {
                                    span: DUMMY_SP,
                                    elems: self
                                        .async_deps_idents
                                        .iter()
                                        .map(|ident| {
                                            Some(ExprOrSpread {
                                                spread: None,
                                                expr: Box::new(Expr::Ident(ident.id.clone())),
                                            })
                                        })
                                        .collect(),
                                })),
                            }],
                            type_args: None,
                        }))),
                        definite: false,
                    }],
                })))),
            );

            // Insert code: `[async1, async2] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;`
            module_items.insert(
                self.last_dep_pos + 2,
                ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                    span: DUMMY_SP,
                    expr: Box::new(Expr::Assign(AssignExpr {
                        span: DUMMY_SP,
                        left: PatOrExpr::Expr(Box::new(Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: self
                                .async_deps_idents
                                .iter()
                                .map(|ident| {
                                    Some(ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Ident(ident.id.clone())),
                                    })
                                })
                                .collect(),
                        }))),
                        right: Box::new(Expr::Cond(CondExpr {
                            span: DUMMY_SP,
                            test: Box::new(Expr::Member(MemberExpr {
                                span: DUMMY_SP,
                                obj: Box::new(Expr::Ident(Ident {
                                    span: DUMMY_SP,
                                    sym: ASYNC_DEPS_IDENT.into(),
                                    optional: false,
                                })),
                                prop: MemberProp::Ident(Ident {
                                    span: DUMMY_SP,
                                    sym: "then".into(),
                                    optional: false,
                                }),
                            })),
                            cons: Box::new(Expr::Call(CallExpr {
                                span: DUMMY_SP,
                                callee: Callee::Expr(Box::new(Expr::Paren(ParenExpr {
                                    span: DUMMY_SP,
                                    expr: Box::new(Expr::Await(AwaitExpr {
                                        span: DUMMY_SP,
                                        arg: Box::new(Expr::Ident(Ident {
                                            span: DUMMY_SP,
                                            sym: ASYNC_DEPS_IDENT.into(),
                                            optional: false,
                                        })),
                                    })),
                                }))),
                                args: vec![],
                                type_args: None,
                            })),
                            alt: Box::new(Expr::Ident(Ident {
                                span: DUMMY_SP,
                                sym: ASYNC_DEPS_IDENT.into(),
                                optional: false,
                            })),
                        })),
                        op: AssignOp::Assign,
                    })),
                })),
            );
        }

        // Insert code: `asyncResult()`
        module_items.push(ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "asyncResult".into(),
                    optional: false,
                }))),
                type_args: None,
                args: vec![],
            })),
        })));

        // Wrap async module with `require._async(module, async (handleAsyncDeps, asyncResult) => { });`
        *module_items = vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
                    span: DUMMY_SP,
                    obj: Box::new(Expr::Ident(self.context.meta.script.require_ident.clone())),
                    prop: MemberProp::Ident(Ident {
                        span: DUMMY_SP,
                        sym: "_async".into(),
                        optional: false,
                    }),
                }))),
                type_args: None,
                args: vec![
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Ident(self.context.meta.script.module_ident.clone())),
                    },
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Arrow(ArrowExpr {
                            is_async: true,
                            is_generator: false,
                            type_params: None,
                            return_type: None,
                            span: DUMMY_SP,
                            params: vec![
                                Pat::Ident(BindingIdent {
                                    id: Ident {
                                        span: DUMMY_SP,
                                        sym: "handleAsyncDeps".into(),
                                        optional: false,
                                    },
                                    type_ann: None,
                                }),
                                Pat::Ident(BindingIdent {
                                    id: Ident {
                                        span: DUMMY_SP,
                                        sym: "asyncResult".into(),
                                        optional: false,
                                    },
                                    type_ann: None,
                                }),
                            ],
                            body: Box::new(BlockStmtOrExpr::BlockStmt(BlockStmt {
                                span: DUMMY_SP,
                                stmts: module_items
                                    .iter()
                                    .map(|stmt| stmt.as_stmt().unwrap().clone())
                                    .collect(),
                            })),
                        })),
                    },
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: if self.top_level_await { "1" } else { "0" }.into(),
                            optional: false,
                        })),
                    },
                ],
            })),
        }))];
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::swc_common::{Globals, DUMMY_SP, GLOBALS};
    use mako_core::swc_ecma_transforms::resolver;
    use mako_core::swc_ecma_visit::VisitMutWith;

    use super::AsyncModule;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::chunk::{Chunk, ChunkType};
    use crate::compiler::Context;
    use crate::module::{Dependency, ModuleId, ResolveType};

    #[test]
    fn test_async_module() {
        let code = r#"
const _async = require('./async');
_async.add(1, 2);
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    const _async = require('./async');
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async
    ]);
    [
        _async
    ] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    _async.add(1, 2);
    asyncResult();
}, 1);

//# sourceMappingURL=index.js.map
            "#
            .trim()
        );
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context: Arc<Context> = Arc::new(Default::default());

        let module_id: ModuleId = "./async".to_string().into();
        let mut chunk = Chunk::new(
            "./async".to_string().into(),
            ChunkType::Entry(module_id, "async".to_string(), false),
        );
        chunk.add_module("./async".to_string().into());

        context.chunk_graph.write().unwrap().add_chunk(chunk);

        let mut ast = build_js_ast(path, origin, &context).unwrap();

        let globals = Globals::default();
        GLOBALS.set(&globals, || {
            let mut async_module = AsyncModule {
                async_deps: &vec![Dependency {
                    resolve_type: ResolveType::Import,
                    source: String::from("./async"),
                    resolve_as: None,
                    span: Some(DUMMY_SP),
                    order: 1,
                }],
                async_deps_idents: Vec::new(),
                last_dep_pos: 0,
                top_level_await: true,
                context: &context,
                unresolved_mark: ast.unresolved_mark,
            };
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            ast.ast.visit_mut_with(&mut async_module);
        });

        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
