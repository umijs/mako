use std::sync::Arc;

use anyhow::Result;
use swc_common::errors::HANDLER;
use swc_common::{Mark, GLOBALS};
use swc_ecma_ast::Module;
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

use crate::build::Task;
use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::transform_dep_replacer::{DepReplacer, DependenciesToReplace};

#[allow(dead_code)]
pub fn transform_after_resolve(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    task: &Task,
    deps_to_replace: &DependenciesToReplace,
) -> Result<()> {
    match ast {
        ModuleAst::Script(ast) => transform_js(
            &mut ast.ast,
            context,
            task,
            ast.top_level_mark,
            ast.unresolved_mark,
            deps_to_replace,
        ),
        _ => Ok(()),
    }
}

#[allow(dead_code)]
fn transform_js(
    ast: &mut Module,
    context: &Arc<Context>,
    _task: &Task,
    _top_level_mark: Mark,
    _unresolved_mark: Mark,
    deps_to_replace: &DependenciesToReplace,
) -> Result<()> {
    let cm = context.meta.script.cm.clone();
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    let mut dep_replacer = DepReplacer {
                        to_replace: deps_to_replace,
                        context,
                    };
                    ast.visit_mut_with(&mut dep_replacer);
                    Ok(())
                })
            })
        })
    })
}
