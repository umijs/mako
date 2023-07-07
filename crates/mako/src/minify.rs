use std::sync::Arc;

use anyhow::Result;
use swc_common::errors::HANDLER;
use swc_common::GLOBALS;
use swc_ecma_minifier::optimize;
use swc_ecma_minifier::option::{ExtraOptions, MinifyOptions};
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_transforms::{fixer, resolver};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

use crate::ast::Ast;
use crate::compiler::Context;

pub fn minify_js(ast: &mut Ast, context: &Arc<Context>) -> Result<()> {
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(
            context.meta.script.cm.clone(),
            Default::default(),
            |handler| {
                HELPERS.set(&Helpers::new(true), || {
                    HANDLER.set(handler, || {
                        let unresolved_mark = ast.unresolved_mark;
                        let top_level_mark = ast.top_level_mark;

                        ast.ast.visit_mut_with(&mut resolver(
                            unresolved_mark,
                            top_level_mark,
                            false,
                        ));

                        let mut minified = optimize(
                            ast.ast.clone().into(),
                            context.meta.script.cm.clone(),
                            Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            ),
                            None,
                            &MinifyOptions {
                                compress: Some(Default::default()),
                                mangle: Some(Default::default()),
                                ..Default::default()
                            },
                            &ExtraOptions {
                                unresolved_mark,
                                top_level_mark,
                            },
                        )
                        .expect_module();

                        minified.visit_mut_with(&mut fixer(Some(
                            context
                                .meta
                                .script
                                .origin_comments
                                .read()
                                .unwrap()
                                .get_swc_comments(),
                        )));

                        ast.ast = minified;
                        Ok(())
                    })
                })
            },
        )
    })
}
