use std::sync::Arc;

use anyhow::Result;
use swc_core::common::errors::HANDLER;
use swc_core::common::GLOBALS;
use swc_core::css::ast::Stylesheet;
use swc_core::css::minifier;
use swc_core::ecma::minifier::optimize;
use swc_core::ecma::minifier::option::{ExtraOptions, MinifyOptions};
use swc_core::ecma::transforms::base::fixer::{fixer, paren_remover};
use swc_core::ecma::transforms::base::helpers::{Helpers, HELPERS};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

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

                        let comments_lock = context.meta.script.origin_comments.read().unwrap();

                        let comments = comments_lock.get_swc_comments();

                        ast.ast.visit_mut_with(&mut resolver(
                            unresolved_mark,
                            top_level_mark,
                            false,
                        ));
                        ast.ast.visit_mut_with(&mut paren_remover(Some(comments)));

                        let mut minified = optimize(
                            ast.ast.clone().into(),
                            context.meta.script.cm.clone(),
                            Some(comments),
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

                        minified.visit_mut_with(&mut fixer(Some(comments)));

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
