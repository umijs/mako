use std::collections::HashSet;

use mako_core::swc_ecma_ast::{
    CallExpr, Callee, Decl, Expr, Lit, Module, ModuleItem, Stmt, VarDecl, VarDeclarator,
};

pub struct SwcHelpers {
    pub helpers: HashSet<String>,
}

impl SwcHelpers {
    pub fn new(helpers: Option<HashSet<String>>) -> Self {
        let helpers = if let Some(helpers) = helpers {
            helpers
        } else {
            HashSet::new()
        };
        Self { helpers }
    }

    pub fn extends(&mut self, helpers: HashSet<String>) {
        self.helpers.extend(helpers);
    }

    pub fn get_helpers(&self) -> Vec<String> {
        self.helpers.iter().map(|h| h.to_string()).collect()
    }

    // for watch mode
    pub fn full_helpers(&self) -> HashSet<String> {
        let mut helpers = HashSet::new();
        helpers.insert("@swc/helpers/_/_interop_require_default".into());
        helpers.insert("@swc/helpers/_/_interop_require_wildcard".into());
        helpers.insert("@swc/helpers/_/_export_star".into());
        helpers
    }

    pub fn get_swc_helpers(ast: &Module) -> HashSet<String> {
        let mut swc_helpers = HashSet::new();
        // Top level require only
        // why top level only? because swc helpers is only used in top level
        // why require only? because cjs transform is done before this
        ast.body.iter().for_each(|stmt| {
            // e.g.
            // var _interop_require_wildcard = __mako_require__("@swc/helpers/_/_interop_require_wildcard");
            if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(box VarDecl { decls, .. }))) = stmt {
                if decls.is_empty() {
                    return;
                }
                let decl = decls.first().unwrap();
                if let VarDeclarator {
                    init: Some(box Expr::Call(CallExpr { callee, args, .. })),
                    ..
                } = decl
                {
                    let is_require = if let Callee::Expr(box Expr::Ident(ident)) = &callee {
                        ident.sym.as_ref() == "__mako_require__"
                    } else {
                        false
                    };
                    if !is_require {
                        return;
                    }
                    if let Some(arg) = args.first() {
                        if let Expr::Lit(Lit::Str(dep)) = arg.expr.as_ref() {
                            let is_swc_helper = dep.value.starts_with("@swc/helpers/_/");
                            if is_swc_helper {
                                swc_helpers.insert(dep.value.to_string());
                            }
                        }
                    }
                }
            }
        });
        swc_helpers
    }
}

impl Default for SwcHelpers {
    fn default() -> Self {
        Self::new(None)
    }
}
