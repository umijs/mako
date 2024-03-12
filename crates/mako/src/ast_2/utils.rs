use mako_core::base64::engine::general_purpose;
use mako_core::base64::Engine;
use mako_core::swc_ecma_ast::{
    CallExpr, Callee, Expr, Ident, Import, Lit, MemberExpr, MemberProp, MetaPropExpr, MetaPropKind,
    Module, ModuleItem,
};
use swc_core::common::Mark;

pub fn base64_encode<T: AsRef<[u8]>>(raw: T) -> String {
    general_purpose::STANDARD.encode(raw)
}

// TODO: more accurate
pub fn is_remote(url: &str) -> bool {
    let lower_url = url.to_lowercase();
    lower_url.starts_with("http://")
        || lower_url.starts_with("https://")
        || lower_url.starts_with("data:")
        || lower_url.starts_with("//")
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
    ident.sym == *sym && ident.span.ctxt.outer() == *unresolved_mark
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
                MemberProp::Ident(Ident {
                    sym,
                    ..
                }),
            ..
        }) if sym == "url"
    )
}
