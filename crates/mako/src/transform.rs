use std::sync::Arc;

use anyhow::Result;
use swc_common::comments::NoopComments;
use swc_common::errors::HANDLER;
use swc_common::sync::Lrc;
use swc_common::{Mark, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_visit::VisitMutWith;
use swc_ecma_ast::Module;
use swc_ecma_preset_env::{self as swc_preset_env};
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::ImportInterop;
use swc_ecma_transforms::typescript::strip_with_jsx;
use swc_ecma_transforms::{resolver, Assumptions};
use swc_ecma_visit::{Fold, VisitMutWith as CssVisitMutWith};
use swc_error_reporters::handler::try_with_handler;

use crate::build::Task;
use crate::compiler::Context;
use crate::config::Mode;
use crate::module::ModuleAst;
use crate::resolve::Resolvers;
use crate::targets;
use crate::transform_css_url_replacer::CSSUrlReplacer;
use crate::transform_env_replacer::{build_env_map, EnvReplacer};
use crate::transform_optimizer::Optimizer;
use crate::transform_provide::Provide;
use crate::transform_react::mako_react;

pub fn transform(
    ast: &mut ModuleAst,
    context: &Arc<Context>,
    task: &Task,
    resolvers: &Resolvers,
) -> Result<()> {
    match ast {
        ModuleAst::Script(ast) => transform_js(
            &mut ast.ast,
            context,
            task,
            ast.top_level_mark,
            ast.unresolved_mark,
        ),
        ModuleAst::Css(ast) => transform_css(ast, context, task, resolvers),
        _ => Ok(()),
    }
}

fn transform_js(
    ast: &mut Module,
    context: &Arc<Context>,
    task: &Task,
    top_level_mark: Mark,
    unresolved_mark: Mark,
) -> Result<()> {
    let cm = context.meta.script.cm.clone();
    let mode = &context.config.mode.to_string();
    let mut define = context.config.define.clone();

    define
        .entry("NODE_ENV".to_string())
        .or_insert_with(|| format!("\"{}\"", mode).into());
    let _is_dev = matches!(context.config.mode, Mode::Development);

    let env_map = build_env_map(define, context)?;
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HELPERS.set(&Helpers::new(true), || {
                HANDLER.set(handler, || {
                    let import_interop = ImportInterop::Swc;

                    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
                    ast.visit_mut_with(&mut strip_with_jsx(
                        cm.clone(),
                        Default::default(),
                        NoopComments,
                        top_level_mark,
                    ));

                    // indent.span needed in mako_react refresh, so it must be after resolver visitor
                    ast.visit_mut_with(&mut mako_react(
                        cm,
                        context,
                        task,
                        &top_level_mark,
                        &unresolved_mark,
                    ));

                    ast.visit_mut_with(&mut import_analyzer(import_interop, true));
                    ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                    let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
                    ast.visit_mut_with(&mut env_replacer);

                    let mut provide = Provide::new(context.config.providers.clone());
                    ast.visit_mut_with(&mut provide);

                    let mut optimizer = Optimizer {};
                    ast.visit_mut_with(&mut optimizer);

                    // TODO: polyfill
                    let mut preset_env = swc_preset_env::preset_env(
                        unresolved_mark,
                        Some(NoopComments),
                        swc_preset_env::Config {
                            mode: Some(swc_preset_env::Mode::Entry),
                            targets: Some(targets::swc_preset_env_targets_from_map(
                                context.config.targets.clone(),
                            )),
                            ..Default::default()
                        },
                        Assumptions::default(),
                        &mut FeatureFlag::default(),
                    );
                    ast.body = preset_env.fold_module(ast.clone()).body;
                    Ok(())
                })
            })
        })
    })
}

fn transform_css(
    ast: &mut Stylesheet,
    context: &Arc<Context>,
    task: &Task,
    resolvers: &Resolvers,
) -> Result<()> {
    let mut css_handler = CSSUrlReplacer {
        resolvers,
        path: &task.path,
        context,
    };
    ast.visit_mut_with(&mut css_handler);
    Ok(())
}
