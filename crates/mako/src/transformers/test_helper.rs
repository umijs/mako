use std::sync::Arc;

use mako_core::swc_common::GLOBALS;
use mako_core::swc_ecma_visit::VisitMut;

use crate::ast::build_js_ast;
use crate::compiler::Context;

pub fn transform_js_code(code: &str, mut visitor: impl VisitMut, context: &Arc<Context>) -> String {
    GLOBALS.set(&context.meta.script.globals, || {
        let mut ast = build_js_ast("test.js", code, context).unwrap();
        crate::test_helper::transform_ast_with(&mut ast.ast, &mut visitor, &context.meta.script.cm)
    })
}
