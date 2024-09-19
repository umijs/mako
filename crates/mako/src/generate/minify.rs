use std::sync::Arc;

use anyhow::Result;
use swc_core::base::try_with_handler;
use swc_core::common::errors::HANDLER;
use swc_core::common::GLOBALS;
use swc_core::css::ast::Stylesheet;
use swc_core::css::minifier;
use swc_core::ecma::minifier::optimize;
use swc_core::ecma::minifier::option::{ExtraOptions, MinifyOptions};
use swc_core::ecma::transforms::base::fixer::fixer;
use swc_core::ecma::transforms::base::helpers::{Helpers, HELPERS};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::visit::VisitMutWith;

use crate::ast::js_ast::JsAst;
use crate::compiler::Context;

pub fn minify_js(ast: &mut JsAst, context: &Arc<Context>) -> Result<()> {
    crate::mako_profile_function!();
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
    crate::mako_profile_function!();
    GLOBALS.set(&context.meta.css.globals, || {
        try_with_handler(context.meta.css.cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    minifier::minify(stylesheet, Default::default());
                    Ok(())
                })
            })
        })
    })
}
