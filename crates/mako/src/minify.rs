use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::GLOBALS;
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_css_minifier;
use mako_core::swc_ecma_minifier::optimize;
use mako_core::swc_ecma_minifier::option::{ExtraOptions, MinifyOptions};
use mako_core::swc_ecma_transforms::fixer::fixer;
use mako_core::swc_ecma_transforms::helpers::{Helpers, HELPERS};
use mako_core::swc_ecma_transforms::resolver;
use mako_core::swc_ecma_visit::VisitMutWith;
use mako_core::swc_error_reporters::handler::try_with_handler;

use crate::ast_2::js_ast::JsAst;
use crate::compiler::Context;

pub fn minify_js(ast: &mut JsAst, context: &Arc<Context>) -> Result<()> {
    mako_core::mako_profile_function!();
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

pub fn minify_css(stylesheet: &mut Stylesheet, context: &Arc<Context>) -> Result<()> {
    mako_core::mako_profile_function!();
    GLOBALS.set(&context.meta.css.globals, || {
        try_with_handler(context.meta.css.cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    swc_css_minifier::minify(stylesheet, Default::default());
                    Ok(())
                })
            })
        })
    })
}
