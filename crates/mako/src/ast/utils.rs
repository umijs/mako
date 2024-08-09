use base64::engine::general_purpose;
use base64::Engine;
use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    CallExpr, Callee, Expr, ExprOrSpread, Ident, IdentName, Import, Lit, MemberExpr, MemberProp,
    MetaPropExpr, MetaPropKind, Module, ModuleItem,
};

pub fn base64_encode<T: AsRef<[u8]>>(raw: T) -> String {
    general_purpose::STANDARD.encode(raw)
}

pub fn is_remote_or_data(url: &str) -> bool {
    let lower_url = url.to_lowercase();
    // ref:
    // https://developer.mozilla.org/en-US/docs/Web/CSS/url
    // https://www.ietf.org/rfc/rfc3986
    lower_url.starts_with("http://")
        || lower_url.starts_with("https://")
        || lower_url.starts_with("data:")
        || lower_url.starts_with("//")
}

pub fn is_remote_or_data_or_hash(url: &str) -> bool {
    let lower_url = url.to_lowercase();
    is_remote_or_data(url)
        // css url() should not resolve hash only url
        // e.g. fill: url(#gradient)
        || lower_url.starts_with('#')
}

pub fn remove_first_tilde(url: String) -> String {
    if let Some(stripped) = url.strip_prefix('~') {
        // ~/ is the case when use ~ as alias or folder
        if url.starts_with("~/") {
            url
        } else {
            stripped.to_string()
        }
    } else {
        url
    }
}

pub fn is_esm(module: &Module) -> bool {
    module
        .body
        .iter()
        .any(|item| matches!(item, ModuleItem::ModuleDecl(_)))
}

pub fn is_dynamic_import(call_expr: &CallExpr) -> bool {
    matches!(&call_expr.callee, Callee::Import(Import { .. }))
}

pub fn is_commonjs_require(call_expr: &CallExpr, unresolved_mark: &Mark) -> bool {
    if let Some(ident) = get_call_expr_ident(call_expr) {
        is_ident_undefined(ident, "require", unresolved_mark)
        // TODO: remove this, it's special logic
        || is_ident(ident, "__mako_require__")
    } else {
        false
    }
}

pub fn get_call_expr_ident(call_expr: &CallExpr) -> Option<&Ident> {
    if let Callee::Expr(box Expr::Ident(ident)) = &call_expr.callee {
        Some(ident)
    } else {
        None
    }
}

pub fn is_ident_undefined(ident: &Ident, sym: &str, unresolved_mark: &Mark) -> bool {
    ident.sym == *sym && ident.ctxt.outer() == *unresolved_mark
}

pub fn get_first_str_arg(call_expr: &CallExpr) -> Option<String> {
    if let Some(arg) = call_expr.args.first() {
        if let box Expr::Lit(Lit::Str(str_)) = &arg.expr {
            return Some(str_.value.to_string());
        }
    }
    None
}

pub fn is_ident(ident: &Ident, sym: &str) -> bool {
    ident.sym == *sym
}

pub fn is_import_meta_url(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Member(MemberExpr {
            obj:
                box Expr::MetaProp(MetaPropExpr {
                    kind: MetaPropKind::ImportMeta,
                    ..
                }),
            prop:
                MemberProp::Ident(IdentName {
                    sym,
                    ..
                }),
            ..
        }) if sym == "url"
    )
}

pub fn id(s: &str) -> Ident {
    Ident {
        ctxt: Default::default(),
        span: DUMMY_SP,
        sym: s.into(),
        optional: false,
    }
}
pub fn member_prop(s: &str) -> MemberProp {
    MemberProp::Ident(IdentName {
        span: DUMMY_SP,
        sym: s.into(),
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
        ctxt: Default::default(),
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
        MemberProp::Ident(id("ensure").into()),
        vec![ExprOrSpread {
            spread: None,
            expr: Box::new(Expr::Lit(Lit::Str(source.into()))),
        }],
    )
}
