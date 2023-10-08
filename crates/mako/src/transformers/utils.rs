use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrowExpr, BlockStmtOrExpr, CallExpr, Callee, Expr, ExprOrSpread, Ident, Lit, MemberExpr,
    MemberProp, Pat,
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

pub fn promise_resolve() -> Expr {
    member_call(Expr::Ident(id("Promise")), member_prop("resolve"), vec![])
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

pub fn call(obj: Expr, args: Vec<ExprOrSpread>) -> Expr {
    Expr::Call(CallExpr {
        span: DUMMY_SP,
        callee: Callee::Expr(Box::new(obj)),
        args,
        type_args: None,
    })
}

pub fn arrow_fn(args: Vec<Pat>, body: Expr) -> Expr {
    Expr::Arrow(ArrowExpr {
        span: DUMMY_SP,
        params: args,
        body: Box::new(BlockStmtOrExpr::Expr(Box::new(body))),
        is_async: false,
        is_generator: false,
        type_params: None,
        return_type: None,
    })
}

pub fn require_ensure(source: String) -> Expr {
    member_call(
        Expr::Ident(id("require")),
        MemberProp::Ident(id("ensure")),
        vec![ExprOrSpread {
            spread: None,
            expr: Box::new(Expr::Lit(Lit::Str(source.into()))),
        }],
    )
}
