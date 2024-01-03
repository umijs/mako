use mako_core::swc_common::util::take::Take;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{BlockStmt, IfStmt, Stmt};
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

pub(super) struct UnSimplify {}

impl VisitMut for UnSimplify {
    fn visit_mut_if_stmt(&mut self, if_stmt: &mut IfStmt) {
        match if_stmt.cons {
            box Stmt::Block(_) => {}
            _ => {
                let cons = if_stmt.cons.take();

                if_stmt.cons = Box::new(
                    BlockStmt {
                        span: DUMMY_SP,
                        stmts: vec![*cons],
                    }
                    .into(),
                );
            }
        }

        if_stmt.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;

    fn context() -> Arc<Context> {
        let mut c: Context = Default::default();
        c.config.devtool = None;

        Arc::new(c)
    }

    #[test]
    fn test_single_stmt_cons() {
        let ctx = context();
        let mut ast = build_js_ast("test.js", "if(1) console.log(1)", &ctx).unwrap();

        ast.ast.visit_mut_with(&mut UnSimplify {});

        let (code, _) = js_ast_to_code(&ast.ast, &ctx, "dist.js").unwrap();

        assert_eq!(
            code,
            r#"if (1) {
    console.log(1);
}
"#
        );
    }

    #[test]
    fn test_if_block_stmt_cons() {
        let ctx = context();
        let mut ast =
            build_js_ast("test.js", "if(1) { console.log(1); console.log(2); }", &ctx).unwrap();

        ast.ast.visit_mut_with(&mut UnSimplify {});

        let (code, _) = js_ast_to_code(&ast.ast, &ctx, "dist.js").unwrap();

        assert_eq!(
            code,
            r#"if (1) {
    console.log(1);
    console.log(2);
}
"#
        );
    }
}
