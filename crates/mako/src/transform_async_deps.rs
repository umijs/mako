use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, AssignExpr, AssignOp, AwaitExpr, BindingIdent, CallExpr, Callee, CondExpr, Decl,
    Expr, ExprOrSpread, ExprStmt, Ident, ImportSpecifier, MemberExpr, MemberProp, ModuleDecl,
    ModuleItem, ParenExpr, Pat, PatOrExpr, Stmt, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_ecma_visit::VisitMut;

use crate::module::Dependency;

const ASYNC_DEPS_IDENT: &str = "__mako_async_dependencies__";

pub struct AsyncDeps<'a> {
    pub async_deps: &'a Vec<Dependency>,
    pub async_deps_specifiers: Vec<ImportSpecifier>,
    pub last_import_pos: usize,
    pub top_level_await: bool,
}

impl VisitMut for AsyncDeps<'_> {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        // 收集所有 async deps 的标识符, 同时记录最后一个 import 语句的位置
        for module_item in module_items.iter_mut() {
            if let ModuleItem::ModuleDecl(module_decl) = &module_item {
                if let ModuleDecl::Import(import_decl) = &module_decl {
                    self.last_import_pos += 1;
                    if import_decl.type_only {
                        break;
                    }

                    if self
                        .async_deps
                        .iter()
                        .any(|dep| import_decl.src.value == dep.source)
                    {
                        // 记录 SPAN
                        self.async_deps_specifiers
                            .append(&mut import_decl.specifiers.clone());
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if !self.async_deps_specifiers.is_empty() {
            // 在最后一个 import 语句后面插入一行代码: `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
            module_items.insert(
                self.last_import_pos,
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
                                        .async_deps_specifiers
                                        .iter()
                                        .map(|specifier| {
                                            Some(ExprOrSpread {
                                                spread: None,
                                                expr: Box::new(Expr::Ident(
                                                    specifier.as_named().unwrap().local.clone(),
                                                )),
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
                self.last_import_pos + 1,
                ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                    span: DUMMY_SP,
                    expr: Box::new(Expr::Assign(AssignExpr {
                        span: DUMMY_SP,
                        left: PatOrExpr::Expr(Box::new(Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: self
                                .async_deps_specifiers
                                .iter()
                                .map(|specifier| {
                                    Some(ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Ident(
                                            specifier.as_named().unwrap().local.clone(),
                                        )),
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
    }
}
