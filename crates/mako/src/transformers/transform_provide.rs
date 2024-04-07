use std::collections::HashMap;

use mako_core::indexmap::IndexMap;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{Expr, Ident, MemberExpr, Module, ModuleItem, VarDeclKind};
use mako_core::swc_ecma_utils::{quote_ident, quote_str, ExprFactory};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_core::common::{Mark, SyntaxContext};

use crate::config::Providers;
pub struct Provide {
    unresolved_mark: Mark,
    top_level_mark: Mark,
    providers: Providers,
    var_decls: IndexMap<String, ModuleItem>,
}
impl Provide {
    pub fn new(providers: Providers, unresolved_mark: Mark, top_level_mark: Mark) -> Self {
        Self {
            unresolved_mark,
            top_level_mark,
            providers,
            var_decls: Default::default(),
        }
    }
}

struct ToTopLevelVars {
    unresolved_mark: Mark,
    replaces_map: HashMap<String, SyntaxContext>,
}

impl VisitMut for Provide {
    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);
        module
            .body
            .splice(0..0, self.var_decls.iter().map(|(_, var)| var.clone()));

        module.visit_mut_with(&mut ToTopLevelVars::new(
            self.unresolved_mark,
            self.top_level_mark,
            &self.var_decls,
        ))
    }
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(Ident { ref sym, span, .. }) = expr {
            let has_binding = span.ctxt.outer() != self.unresolved_mark;
            let name = &sym.to_string();
            let provider = self.providers.get(name);
            if !has_binding && provider.is_some() {
                if let Some((from, key)) = provider {
                    let require_decl: ModuleItem = {
                        if key.is_empty() {
                            // eg: const process = require('process');
                            quote_ident!("__mako_require__")
                                .as_call(DUMMY_SP, vec![quote_str!(from.as_str()).as_arg()])
                                .into_var_decl(
                                    VarDeclKind::Const,
                                    quote_ident!(*span, sym.clone()).into(),
                                )
                                .into()
                        } else {
                            // require("buffer")
                            let require_expr = quote_ident!("__mako_require__")
                                .as_call(DUMMY_SP, vec![quote_str!(from.as_str()).as_arg()]);

                            // eg const Buffer = require("buffer").Buffer;
                            Expr::Member(MemberExpr {
                                obj: require_expr.into(),
                                span: DUMMY_SP,
                                prop: quote_ident!(key.as_str()).into(),
                            })
                            .into_var_decl(
                                VarDeclKind::Const,
                                quote_ident!(*span, sym.clone()).into(),
                            )
                            .into()
                        }
                    };

                    self.var_decls.insert(name.to_string(), require_decl);
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

impl ToTopLevelVars {
    fn new(
        unresolved_mark: Mark,
        top_level_mark: Mark,
        vars: &IndexMap<String, ModuleItem>,
    ) -> Self {
        let mut replaces: HashMap<String, SyntaxContext> = Default::default();

        vars.iter().for_each(|(k, _)| {
            let ctxt = SyntaxContext::empty().apply_mark(top_level_mark);
            replaces.insert(k.clone(), ctxt);
        });

        Self {
            unresolved_mark,
            replaces_map: replaces,
        }
    }
}

impl VisitMut for ToTopLevelVars {
    fn visit_mut_ident(&mut self, i: &mut Ident) {
        if i.span.ctxt.outer() == self.unresolved_mark {
            if let Some(ctxt) = self.replaces_map.get(&i.sym.to_string()) {
                i.span.ctxt = *ctxt;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::resolver;
    use swc_core::ecma::visit::VisitMutWith;

    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::test_helper::emit_js;
    use crate::transformers::transform_provide::Provide;

    #[test]
    fn test_provide_normal() {
        // TODO: fix binding test problem
        crate::assert_display_snapshot!(transform(
            r#"
console.log(process);
console.log(process.env);
Buffer.from('foo');
function foo() {
    // let process = 1;
    // console.log(process);
    // let Buffer = 'b';
    // Buffer.from('foo');
}
            "#,
        ));
    }

    fn transform(code: &str) -> String {
        let context: Arc<Context> = Arc::new(Default::default());
        let mut providers = HashMap::new();
        providers.insert("process".into(), ("process".into(), "".into()));
        providers.insert("Buffer".into(), ("buffer".into(), "Buffer".into()));

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("test.js", code, &context).unwrap();
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));

            let mut visitor = Provide::new(providers, ast.unresolved_mark, ast.top_level_mark);
            ast.ast.visit_mut_with(&mut visitor);

            emit_js(&ast.ast, &context.meta.script.cm)
        })
    }
}
