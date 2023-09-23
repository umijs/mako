use swc_atoms::JsWord;
use swc_common::collections::{AHashMap, AHashSet};
use swc_common::sync::{Lock, Lrc};
use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    BindingIdent, CallExpr, Callee, Decl, Expr, ExprOrSpread, Id, Ident, Lit, MemberExpr,
    MemberProp, Module, ModuleItem, Pat, Stmt, Str, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::config::Providers;

pub struct Provide {
    bindings: Lrc<AHashSet<Id>>,
    providers: Providers,
    var_decls: Lock<AHashMap<String, ModuleItem>>,
}

impl Provide {
    pub fn new(providers: Providers) -> Self {
        Self {
            bindings: Default::default(),
            providers,
            var_decls: Default::default(),
        }
    }
}

impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.bindings = Lrc::new(collect_decls(&*module));
        module.visit_mut_children_with(self);

        module.body.splice(
            0..0,
            self.var_decls.borrow().iter().map(|(_, var)| var.clone()),
        );
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = self.bindings.contains(&(sym.clone(), span.ctxt));
            let provider = self.providers.get(&sym.to_string());
            if !has_binding && provider.is_some() {
                let (from, key) = provider.unwrap();

                // require("provider")
                let require_call = Expr::Call(CallExpr {
                    span: *span,
                    callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                        span: *span,
                        sym: "require".into(),
                        optional: false,
                    }))),
                    args: vec![ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Lit(Lit::Str(Str {
                            span: DUMMY_SP,
                            value: JsWord::from(from.clone()),
                            raw: None,
                        }))),
                    }],
                    type_args: None,
                });

                let require_decl = {
                    if key.is_empty() {
                        // const process = require('process');
                        VarDeclarator {
                            span: *span,
                            name: Pat::Ident(BindingIdent {
                                id: Ident {
                                    span: *span,
                                    sym: from.as_str().into(),
                                    optional: false,
                                },
                                type_ann: None,
                            }),
                            init: Some(Box::new(require_call)),
                            definite: false,
                        }
                    } else {
                        // const Buffer = require("buffer").Buffer;
                        VarDeclarator {
                            span: *span,
                            name: Pat::Ident(BindingIdent {
                                id: Ident {
                                    span: *span,
                                    sym: key.as_str().into(),
                                    optional: false,
                                },
                                type_ann: None,
                            }),
                            init: Some(Box::new(Expr::Member(MemberExpr {
                                obj: Box::new(require_call),
                                span: DUMMY_SP,
                                prop: MemberProp::Ident(Ident {
                                    span: *span,
                                    sym: key.as_str().into(),
                                    optional: false,
                                }),
                            }))),
                            definite: false,
                        }
                    }
                };

                self.var_decls.borrow_mut().insert(
                    key.clone(),
                    ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(VarDecl {
                        span: *span,
                        kind: VarDeclKind::Const,
                        declare: false,
                        decls: vec![require_decl],
                    })))),
                );
            }
        }

        expr.visit_mut_children_with(self);
    }
}
