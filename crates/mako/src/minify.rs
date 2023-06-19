use anyhow::Result;
use swc_common::errors::HANDLER;
use swc_common::sync::Lrc;
use swc_common::{Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::Module;
use swc_ecma_minifier::optimize;
use swc_ecma_minifier::option::{ExtraOptions, MinifyOptions};
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_transforms::{fixer, resolver};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

pub fn minify_js(mut ast: Module, cm: &Lrc<SourceMap>) -> Result<Module> {
    let globals = Globals::default();
    GLOBALS.set(&globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    let unresolved_mark = Mark::new();
                    let top_level_mark = Mark::new();

                    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));

                    let mut minified = optimize(
                        ast.into(),
                        cm.clone(),
                        None,
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

                    minified.visit_mut_with(&mut fixer(None));

                    Ok(minified)
                })
            })
        })
    })
}
