// a workaround for the issue https://github.com/umijs/mako/issues/1274
//                            https://github.com/swc-project/swc/issues/9045

use std::collections::HashSet;

use swc_core::common::{Mark, SyntaxContext};
use swc_core::ecma::ast::{Id, Ident, Module};
use swc_core::ecma::utils::IdentRenamer;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub(crate) struct FixSymbolConflict {
    idents_named_symbol: HashSet<Id>,
    top_level_ctxt: SyntaxContext,
}

impl FixSymbolConflict {
    pub fn new(top_level_mark: Mark) -> Self {
        Self {
            idents_named_symbol: Default::default(),
            top_level_ctxt: SyntaxContext::empty().apply_mark(top_level_mark),
        }
    }
}

impl VisitMut for FixSymbolConflict {
    fn visit_mut_ident(&mut self, n: &mut Ident) {
        if n.sym.eq("Symbol") && n.ctxt == self.top_level_ctxt {
            self.idents_named_symbol.insert(n.to_id());
        }
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        n.visit_mut_children_with(self);

        if !self.idents_named_symbol.is_empty() {
            let rename_map = self
                .idents_named_symbol
                .iter()
                .map(|id| {
                    let new_sym = format!("_$m_{}", id.0);
                    (id.clone(), (new_sym.into(), id.1))
                })
                .collect();

            let mut renamer = IdentRenamer::new(&rename_map);

            n.visit_mut_with(&mut renamer);
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_global_symbol() {
        assert_eq!(
            run_with("console.log(Symbol.iterator)"),
            "console.log(Symbol.iterator);"
        );
    }

    #[test]
    fn test_top_level_redefine_symbol() {
        assert_eq!(
            run_with("class Symbol {} Symbol.iterator; export { Symbol }"),
            r#"
class _$m_Symbol {
}
_$m_Symbol.iterator;
export { _$m_Symbol as Symbol };
"#
            .trim()
        )
    }

    #[test]
    fn test_redefine_symbol_in_nested_scope() {
        assert_eq!(
            run_with(
                r#"
Symbol.iterator;        
(function(){
    class Symbol {}
})();"#,
            ),
            r#"
Symbol.iterator;
(function() {
    class Symbol1 {
    }
})();            
"#
            .trim()
        );
    }

    fn run_with(code: &str) -> String {
        let mut tu = TestUtils::gen_js_ast(code);
        let mark = tu.ast.js().top_level_mark;
        let mut v = GLOBALS.set(&tu.context.meta.script.globals, || {
            FixSymbolConflict::new(mark)
        });
        tu.ast.js_mut().ast.visit_mut_with(&mut v);
        tu.js_ast_to_code()
    }
}
