use std::sync::Arc;

use anyhow::Result;
use swc_core::base::try_with_handler;
use swc_core::common::errors::HANDLER;
use swc_core::common::{Mark, GLOBALS};
use swc_core::css::ast::{AtRule, AtRulePrelude, ImportHref, Rule, Str, Stylesheet, UrlValue};
use swc_core::css::compat::compiler::{self, Compiler};
use swc_core::css::{compat as swc_css_compat, prefixer, visit as swc_css_visit};
use swc_core::ecma::ast::Module;
use swc_core::ecma::preset_env::{self as swc_preset_env};
use swc_core::ecma::transforms::base::feature::FeatureFlag;
use swc_core::ecma::transforms::base::fixer::paren_remover;
use swc_core::ecma::transforms::base::helpers::{Helpers, HELPERS};
use swc_core::ecma::transforms::base::{resolver, Assumptions};
use swc_core::ecma::transforms::compat::reserved_words;
use swc_core::ecma::transforms::optimization::simplifier;
use swc_core::ecma::transforms::optimization::simplify::{dce, Config as SimplifyConfig};
use swc_core::ecma::transforms::proposal::decorators;
use swc_core::ecma::visit::{Fold, VisitMut, VisitMutWith};

use crate::ast::css_ast::CssAst;
use crate::ast::file::File;
use crate::build::targets;
use crate::build::targets::swc_preset_env_targets_from_map;
use crate::compiler::Context;
use crate::config::Mode;
use crate::features;
use crate::module::ModuleAst;
use crate::plugin::PluginTransformJsParam;
use crate::plugins::context_module::ContextModuleVisitor;
use crate::visitors::amd_define_overrides::amd_define_overrides;
use crate::visitors::clean_ctxt::clean_syntax_context;
use crate::visitors::css_assets::CSSAssets;
use crate::visitors::css_flexbugs::CSSFlexbugs;
use crate::visitors::css_px2rem::Px2Rem;
use crate::visitors::default_export_namer::DefaultExportNamer;
use crate::visitors::dynamic_import_to_require::DynamicImportToRequire;
use crate::visitors::env_replacer::{build_env_map, EnvReplacer};
use crate::visitors::fix_helper_inject_position::FixHelperInjectPosition;
use crate::visitors::fix_symbol_conflict::FixSymbolConflict;
use crate::visitors::import_meta_env_replacer::ImportMetaEnvReplacer;
use crate::visitors::import_template_to_string_literal::ImportTemplateToStringLiteral;
use crate::visitors::new_url_assets::NewUrlAssets;
use crate::visitors::provide::Provide;
use crate::visitors::public_path_assignment::PublicPathAssignment;
use crate::visitors::react::react;
use crate::visitors::try_resolve::TryResolve;
use crate::visitors::ts_strip::ts_strip;
use crate::visitors::tsx_strip::tsx_strip;
use crate::visitors::virtual_css_modules::VirtualCSSModules;
use crate::visitors::worker_module::WorkerModule;

pub struct Transform {}

impl Transform {
    pub fn transform(ast: &mut ModuleAst, file: &File, context: Arc<Context>) -> Result<()> {
        crate::mako_profile_function!();
        match ast {
            ModuleAst::Script(ast) => {
                let cm = context.meta.script.cm.clone();
                GLOBALS.set(&context.meta.script.globals, || {
                    try_with_handler(cm, Default::default(), |handler| {
                        HELPERS.set(&Helpers::new(true), || {
                            HANDLER.set(handler, || {
                                let unresolved_mark = ast.unresolved_mark;
                                let top_level_mark = ast.top_level_mark;
                                let cm: Arc<swc_core::common::SourceMap> =
                                    context.meta.script.cm.clone();
                                let origin_comments =
                                    context.meta.script.origin_comments.read().unwrap();
                                let is_ts = file.extname == "ts";
                                let is_tsx = file.extname == "tsx";
                                let is_jsx = file.is_content_jsx()
                                    || file.extname == "jsx"
                                    || file.extname == "js"
                                    || file.extname == "ts"
                                    || file.extname == "tsx";

                                if is_tsx || is_ts {
                                    if is_tsx {
                                        strip_unresolved_tsx(
                                            &mut ast.ast,
                                            context.clone(),
                                            cm.clone(),
                                        )
                                    }
                                    // strip should be ts only
                                    // since when use this in js, it will remove all unused imports
                                    // which is not expected as what webpack does
                                    if is_ts {
                                        strip_unresolved_ts(&mut ast.ast);
                                    }
                                }

                                // visitors
                                let mut visitors: Vec<Box<dyn VisitMut>> = vec![
                                    Box::new(resolver(
                                        unresolved_mark,
                                        top_level_mark,
                                        is_ts || is_tsx,
                                    )),
                                    // fix helper inject position
                                    // should be removed after upgrade to latest swc
                                    // ref: https://github.com/umijs/mako/issues/1193
                                    Box::new(FixHelperInjectPosition::new()),
                                    Box::new(FixSymbolConflict::new(top_level_mark)),
                                    Box::new(NewUrlAssets {
                                        context: context.clone(),
                                        path: file.path.clone(),
                                        unresolved_mark,
                                    }),
                                    Box::new(WorkerModule::new(unresolved_mark)),
                                ];

                                // named default export
                                if context.args.watch && !file.is_under_node_modules && is_jsx {
                                    visitors.push(Box::new(DefaultExportNamer::new()));
                                }
                                // react & react-refresh
                                let is_dev = matches!(context.config.mode, Mode::Development);
                                let is_browser = matches!(
                                    context.config.platform,
                                    crate::config::Platform::Browser
                                );
                                let use_refresh = is_dev
                                    && context.args.watch
                                    && context.config.hmr.is_some()
                                    && !file.is_under_node_modules
                                    && is_browser;
                                if is_jsx {
                                    visitors.push(react(
                                        cm.clone(),
                                        context.clone(),
                                        use_refresh,
                                        &top_level_mark,
                                        &unresolved_mark,
                                    ));
                                }
                                {
                                    let mut define = context.config.define.clone();
                                    let mode = context.config.mode.to_string();
                                    define
                                        .entry("process.env.NODE_ENV".to_string())
                                        .or_insert_with(|| format!("\"{}\"", mode).into());
                                    let env_map = build_env_map(define, &context)?;
                                    visitors
                                        .push(Box::new(EnvReplacer::new(env_map, unresolved_mark)));
                                    visitors.push(Box::new(ImportMetaEnvReplacer::new(mode)));
                                }
                                visitors.push(Box::new(TryResolve {
                                    path: file.path.to_string_lossy().to_string(),
                                    context: context.clone(),
                                    unresolved_mark,
                                }));
                                visitors.push(Box::new(PublicPathAssignment { unresolved_mark }));
                                // TODO: refact provide
                                visitors.push(Box::new(Provide::new(
                                    context.config.providers.clone(),
                                    unresolved_mark,
                                    top_level_mark,
                                )));
                                visitors.push(Box::new(VirtualCSSModules {
                                    auto_css_modules: context.config.auto_css_modules,
                                    unresolved_mark,
                                }));
                                // TODO: move ContextModuleVisitor out of plugin
                                visitors.push(Box::new(ContextModuleVisitor { unresolved_mark }));
                                visitors.push(Box::new(ImportTemplateToStringLiteral {}));
                                // DynamicImportToRequire must be after ContextModuleVisitor
                                // since ContextModuleVisitor will add extra dynamic imports
                                if context.config.dynamic_import_to_require {
                                    visitors.push(Box::new(DynamicImportToRequire::new(
                                        unresolved_mark,
                                    )));
                                }
                                if matches!(context.config.platform, crate::config::Platform::Node)
                                {
                                    visitors.push(Box::new(
                                        features::node::MockFilenameAndDirname {
                                            unresolved_mark,
                                            current_path: file.path.clone(),
                                            context: context.clone(),
                                        },
                                    ));
                                }

                                // folders
                                let mut folders: Vec<Box<dyn Fold>> = vec![];
                                // decorators should go before preset_env, when compile down to es5,
                                // classes become functions, then the decorators on the functions
                                // will be removed silently.
                                folders.push(Box::new(decorators(decorators::Config {
                                    legacy: true,
                                    emit_metadata: context.config.emit_decorator_metadata,
                                    ..Default::default()
                                })));
                                let comments = origin_comments.get_swc_comments().clone();
                                let assumptions = context.assumptions_for(file);

                                // NOTICE: remove optimize_package_imports temporarily
                                // folders.push(Box::new(Optional {
                                //     enabled: should_optimize(file.path.to_str().unwrap(), context.clone()),
                                //     visitor: optimize_package_imports(
                                //         file.path.to_string_lossy().to_string(),
                                //         context.clone(),
                                //     ),
                                // }));

                                ast.transform(&mut visitors, &mut folders, false, context.clone())?;

                                // transform with plugin
                                context.plugin_driver.transform_js(
                                    &PluginTransformJsParam {
                                        handler,
                                        path: file.path.to_str().unwrap(),
                                        top_level_mark,
                                        unresolved_mark,
                                    },
                                    &mut ast.ast,
                                    &context,
                                )?;

                                // preset_env should go last
                                let mut preset_folders: Vec<Box<dyn Fold>> = vec![
                                    Box::new(swc_preset_env::preset_env(
                                        unresolved_mark,
                                        Some(comments),
                                        swc_preset_env::Config {
                                            mode: Some(swc_preset_env::Mode::Entry),
                                            targets: Some(swc_preset_env_targets_from_map(
                                                context.config.targets.clone(),
                                            )),
                                            ..Default::default()
                                        },
                                        assumptions,
                                        &mut FeatureFlag::default(),
                                    )),
                                    Box::new(reserved_words::reserved_words()),
                                    Box::new(paren_remover(Default::default())),
                                    // simplify, but keep top level dead code
                                    // e.g. import x from 'foo'; but x is not used
                                    // this must be kept for tree shaking to work
                                    Box::new(simplifier(
                                        unresolved_mark,
                                        SimplifyConfig {
                                            dce: dce::Config {
                                                top_level: false,
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        },
                                    )),
                                    Box::new(amd_define_overrides(unresolved_mark)),
                                ];
                                ast.transform(
                                    &mut vec![],
                                    &mut preset_folders,
                                    true,
                                    context.clone(),
                                )
                            })
                        })
                    })?;
                    Ok(())
                })
            }
            ModuleAst::Css(ast) => {
                // replace @import url() to @import before CSSUrlReplacer
                import_url_to_href(&mut ast.ast);
                let mut visitors: Vec<Box<dyn swc_css_visit::VisitMut>> = vec![];
                visitors.push(Box::new(Compiler::new(compiler::Config {
                    process: swc_css_compat::feature::Features::NESTING,
                })));
                let path = file.path.to_string_lossy().to_string();
                visitors.push(Box::new(CSSAssets {
                    path,
                    context: context.clone(),
                }));
                // same ability as postcss-flexbugs-fixes
                if context.config.flex_bugs {
                    visitors.push(Box::new(CSSFlexbugs {}));
                }
                if context.config.px2rem.is_some() {
                    let context = context.clone();
                    visitors.push(Box::new(Px2Rem::new(
                        context.config.px2rem.as_ref().unwrap().clone(),
                    )));
                }
                // prefixer
                visitors.push(Box::new(prefixer::prefixer(prefixer::options::Options {
                    env: Some(targets::swc_preset_env_targets_from_map(
                        context.config.targets.clone(),
                    )),
                })));
                ast.transform(&mut visitors)?;

                // css modules
                let is_modules = file.has_param("modules");
                if is_modules {
                    CssAst::compile_css_modules(file.pathname.to_str().unwrap(), &mut ast.ast);
                }

                Ok(())
            }
            ModuleAst::None => Ok(()),
        }
    }
}

impl Context {
    pub fn assumptions_for(&self, file: &File) -> Assumptions {
        let is_ts = file.extname == "ts" || file.extname == "tsx";

        let mut assumptions = Assumptions::default();
        assumptions.set_public_class_fields |= !self.config.use_define_for_class_fields;
        if is_ts {
            assumptions.set_class_methods |= !self.config.use_define_for_class_fields;
        }
        assumptions
    }
}

// TODO: use visitor instead
// Why do this?
// 为了修复 @import url() 会把 css 当 asset 处理，返回 base64 的问题
// 把 @import url() 转成 @import 之后，所有 url() 就都是 rule 里的了
// e.g. @import url("foo") => @import "foo"
fn import_url_to_href(ast: &mut Stylesheet) {
    ast.rules.iter_mut().for_each(|rule| {
        if let Rule::AtRule(box AtRule {
            prelude: Some(box AtRulePrelude::ImportPrelude(preclude)),
            ..
        }) = rule
        {
            if let box ImportHref::Url(url) = &mut preclude.href {
                let href_string = url
                    .value
                    .as_ref()
                    .map(|box value| match value {
                        UrlValue::Str(str) => str.value.to_string(),
                        UrlValue::Raw(raw) => raw.value.to_string(),
                    })
                    .unwrap_or_default();
                preclude.href = Box::new(ImportHref::Str(Str {
                    span: url.span,
                    value: href_string.into(),
                    raw: None,
                }));
            }
        }
    });
}

fn strip_unresolved_ts(ast: &mut Module) {
    let unresolved_mark = Mark::new();
    let top_level_mark = Mark::new();

    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, true));
    ast.visit_mut_with(&mut ts_strip(unresolved_mark, top_level_mark));

    ast.visit_mut_with(&mut clean_syntax_context());
}

fn strip_unresolved_tsx(
    ast: &mut Module,
    context: Arc<Context>,
    cm: Arc<swc_core::common::SourceMap>,
) {
    let unresolved_mark = Mark::new();
    let top_level_mark = Mark::new();

    ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, true));
    ast.visit_mut_with(&mut tsx_strip(cm, context, top_level_mark, unresolved_mark));

    ast.visit_mut_with(&mut clean_syntax_context());
}
