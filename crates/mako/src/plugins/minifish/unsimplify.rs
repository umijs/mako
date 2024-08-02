use swc_core::common::util::take::Take;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{BlockStmt, IfStmt, Stmt};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::DUMMY_CTXT;

pub(super) struct UnSimplify {}

impl VisitMut for UnSimplify {
    fn visit_mut_if_stmt(&mut self, if_stmt: &mut IfStmt) {
        match if_stmt.cons {
            box Stmt::Block(_) => {}
            _ => {
                let cons = if_stmt.cons.take();

                if_stmt.cons = Box::new(
                    BlockStmt {
                        ctxt: DUMMY_CTXT,
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
    use crate::ast::js_ast::JsAst;
    use crate::compiler::Context;

    fn context() -> Arc<Context> {
        let mut c: Context = Default::default();
        c.config.devtool = None;

        Arc::new(c)
    }

    #[test]
    fn test_single_stmt_cons() {
        let ctx = context();
        let mut ast = JsAst::build("test.js", "if(1) console.log(1)", ctx.clone()).unwrap();
        ast.ast.visit_mut_with(&mut UnSimplify {});
        let code = ast.generate(ctx.clone()).unwrap().code;
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
        let mut ast = JsAst::build(
            "test.js",
            "if(1) { console.log(1); console.log(2); }",
            ctx.clone(),
        )
        .unwrap();
        ast.ast.visit_mut_with(&mut UnSimplify {});
        let code = ast.generate(ctx.clone()).unwrap().code;
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
