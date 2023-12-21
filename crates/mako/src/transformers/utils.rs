use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, MemberExpr, MemberProp,
};

pub fn id(s: &str) -> Ident {
    Ident {
        span: DUMMY_SP,
        sym: s.into(),
        optional: false,
    }
}
pub fn member_prop(s: &str) -> MemberProp {
    MemberProp::Ident(Ident {
        span: DUMMY_SP,
        sym: s.into(),
        optional: false,
    })
}

pub fn promise_all(promises: ExprOrSpread) -> Expr {
    member_call(
        Expr::Ident(id("Promise")),
        member_prop("all"),
        vec![promises],
    )
}

pub fn member_call(obj: Expr, member_prop: MemberProp, args: Vec<ExprOrSpread>) -> Expr {
    Expr::Call(CallExpr {
        span: DUMMY_SP,
        callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(obj),
            prop: member_prop,
        }))),
        args,
        type_args: None,
    })
}

pub fn require_ensure(source: String) -> Expr {
    member_call(
        Expr::Ident(id("__mako_require__")),
        MemberProp::Ident(id("ensure")),
        vec![ExprOrSpread {
            spread: None,
            expr: Box::new(Expr::Lit(Lit::Str(source.into()))),
        }],
    )
}
