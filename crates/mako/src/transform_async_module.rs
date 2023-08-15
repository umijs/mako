use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, ArrowExpr, AssignExpr, AssignOp, AwaitExpr, BindingIdent, BlockStmt, BlockStmtOrExpr,
    CallExpr, Callee, CondExpr, Decl, Expr, ExprOrSpread, ExprStmt, Ident, Lit, MemberExpr,
    MemberProp, ModuleItem, ParenExpr, Pat, PatOrExpr, Stmt, Str, VarDecl, VarDeclKind,
    VarDeclarator,
};
use swc_ecma_visit::VisitMut;

use crate::module::Dependency;

const ASYNC_DEPS_IDENT: &str = "__mako_async_dependencies__";
const ASYNC_IMPORTED_MODULE: &str = "_async__mako_imported_module_";

pub struct AsyncModule<'a> {
    pub async_deps: &'a Vec<Dependency>,
    pub async_deps_idents: Vec<BindingIdent>,
    pub last_dep_pos: usize,
    pub top_level_await: bool,
}

impl VisitMut for AsyncModule<'_> {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        // 收集所有 async deps 的标识符, 同时记录最后一个 import 语句的位置
        for (i, module_item) in module_items.iter_mut().enumerate() {
            if let ModuleItem::Stmt(stmt) = module_item {
                match stmt {
                    // `require('./async');` => `var _async__mako_imported_module_n__ = require('./async');`
                    Stmt::Expr(expr_stmt) => {
                        if let Expr::Call(call_expr) = &*expr_stmt.expr {
                            if let Callee::Expr(box Expr::Ident(Ident { sym, .. })) =
                                &call_expr.callee
                            {
                                if sym == "require" {
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
                    }

                    // `var async = require('./async');`
                    Stmt::Decl(Decl::Var(var_decl)) => {
                        for decl in &var_decl.decls {
                            if let Some(box Expr::Call(call_expr)) = &decl.init {
                                if let Callee::Expr(box Expr::Ident(Ident { sym, .. })) =
                                    &call_expr.callee
                                {
                                    if sym == "require" {
                                        if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                            &*call_expr.args[0].expr
                                        {
                                            let source = value.to_string();
                                            if self
                                                .async_deps
                                                .iter()
                                                .any(|dep| dep.source == source)
                                            {
                                                // filter the async deps
                                                if let Pat::Ident(binding_ident) = &decl.name {
                                                    self.async_deps_idents
                                                        .push(binding_ident.clone());
                                                    self.last_dep_pos = i;
                                                }
                                            }
                                        }
                                    }
                                } else if let Callee::Expr(box Expr::Member(MemberExpr {
                                    obj,
                                    prop,
                                    ..
                                })) = &call_expr.callee
                                {
                                    if let Some(Ident { sym, .. }) = obj.clone().ident() {
                                        if &sym == "_interop_require_default"
                                            || &sym == "_interop_require_wildcard"
                                        {
                                            if let Some(Ident { sym, .. }) = prop.clone().ident() {
                                                if &sym == "_" {
                                                    if let Expr::Call(call_expr) =
                                                        &*call_expr.args[0].expr
                                                    {
                                                        if let Callee::Expr(box Expr::Ident(
                                                            Ident { sym, .. },
                                                        )) = &call_expr.callee
                                                        {
                                                            if sym == "require" {
                                                                if let Expr::Lit(Lit::Str(Str {
                                                                    value,
                                                                    ..
                                                                })) = &*call_expr.args[0].expr
                                                                {
                                                                    let source = value.to_string();
                                                                    if self.async_deps.iter().any(
                                                                        |dep| dep.source == source,
                                                                    ) {
                                                                        // filter the async deps
                                                                        if let Pat::Ident(
                                                                            binding_ident,
                                                                        ) = &decl.name
                                                                        {
                                                                            self.async_deps_idents
                                                                                .push(
                                                                                    binding_ident
                                                                                        .clone(),
                                                                                );
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
                    }
                    _ => {}
                }
            }
        }

        if !self.async_deps_idents.is_empty() {
            // 在最后一个 import 语句后面插入一行代码: `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
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

            // 插入代码: `[async1, async2] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;`
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

        // 插入代码 `asyncResult()`
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

        // wrap async module with `require._async(module, async (handleAsyncDeps, asyncResult) => { });`
        *module_items = vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "require._async".into(),
                    optional: false,
                }))),
                type_args: None,
                args: vec![
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: "module".into(),
                            optional: false,
                        })),
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
