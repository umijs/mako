use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_css_ast::{AtRule, AtRulePrelude, ImportHref, Rule, Str, Stylesheet, UrlValue};
use mako_core::swc_css_compat::compiler::{self, Compiler};
use mako_core::swc_ecma_preset_env::{self as swc_preset_env};
use mako_core::swc_ecma_transforms::feature::FeatureFlag;
use mako_core::swc_ecma_transforms::{resolver, Assumptions};
use mako_core::swc_ecma_transforms_optimization::simplifier;
use mako_core::swc_ecma_transforms_optimization::simplify::{dce, Config as SimpilifyConfig};
use mako_core::swc_ecma_transforms_proposals::decorators;
use mako_core::swc_ecma_transforms_typescript::strip_with_jsx;
use mako_core::swc_ecma_visit::{Fold, VisitMut};
use mako_core::{swc_css_compat, swc_css_prefixer, swc_css_visit};
use swc_core::common::GLOBALS;

use crate::ast_2::css_ast::CssAst;
use crate::ast_2::file::File;
use crate::compiler::Context;
use crate::module::ModuleAst;
use crate::targets;
use crate::transformers::transform_css_flexbugs::CSSFlexbugs;
use crate::transformers::transform_css_url_replacer::CSSUrlReplacer;
use crate::transformers::transform_dynamic_import_to_require::DynamicImportToRequire;
use crate::transformers::transform_env_replacer::{build_env_map, EnvReplacer};
use crate::transformers::transform_provide::Provide;
use crate::transformers::transform_px2rem::Px2Rem;
use crate::transformers::transform_react::mako_react;
use crate::transformers::transform_try_resolve::TryResolve;
use crate::transformers::transform_virtual_css_modules::VirtualCSSModules;
use crate::visitors::default_export_namer::DefaultExportNamer;

pub struct Transform {}

impl Transform {
    pub fn transform(ast: &mut ModuleAst, file: &File, context: Arc<Context>) -> Result<()> {
        mako_core::mako_profile_function!();
        match ast {
            ModuleAst::Script(ast) => {
                GLOBALS.set(&context.meta.script.globals, || {
                    let unresolved_mark = ast.unresolved_mark;
                    let top_level_mark = ast.top_level_mark;
                    let cm = context.meta.script.cm.clone();
                    let origin_comments = context.meta.script.origin_comments.read().unwrap();
                    let is_ts = file.extname == "ts" || file.extname == "tsx";

                    // visitors
                    let mut visitors: Vec<Box<dyn VisitMut>> = vec![];
                    visitors.push(Box::new(resolver(unresolved_mark, top_level_mark, false)));
                    // strip should be ts only
                    // since when use this in js, it will remove all unused imports
                    // which is not expected as what webpack does
                    if is_ts {
                        let comments = origin_comments.get_swc_comments().clone();
                        visitors.push(Box::new(strip_with_jsx(
                            cm.clone(),
                            Default::default(),
                            comments,
                            top_level_mark,
                        )))
                    }
                    // named default export
                    let could_be_react_component =
                        file.extname == "tsx" || file.extname == "jsx" || file.extname == "js";
                    if context.args.watch && !file.is_under_node_modules && could_be_react_component
                    {
                        visitors.push(Box::new(DefaultExportNamer::new()));
                    }
                    // TODO: refact mako_react
                    visitors.push(Box::new(mako_react(
                        cm,
                        context.clone(),
                        file,
                        &top_level_mark,
                        &unresolved_mark,
                    )));
                    // TODO: refact env replacer
                    {
                        let mut define = context.config.define.clone();
                        let mode = context.config.mode.to_string();
                        define
                            .entry("NODE_ENV".to_string())
                            .or_insert_with(|| format!("\"{}\"", mode).into());
                        let env_map = build_env_map(define, &context)?;
                        visitors.push(Box::new(EnvReplacer::new(
                            Lrc::new(env_map),
                            unresolved_mark,
                        )));
                    }
                    visitors.push(Box::new(TryResolve {
                        path: file.path.to_string_lossy().to_string(),
                        context: context.clone(),
                        unresolved_mark,
                    }));
                    // TODO: refact provide
                    visitors.push(Box::new(Provide::new(
                        context.config.providers.clone(),
                        unresolved_mark,
                        top_level_mark,
                    )));
                    visitors.push(Box::new(VirtualCSSModules {
                        context: context.clone(),
                        unresolved_mark,
                    }));
                    if context.config.dynamic_import_to_require {
                        visitors.push(Box::new(DynamicImportToRequire { unresolved_mark }));
                    }

                    // folders
                    let mut folders: Vec<Box<dyn Fold>> = vec![];
                    // TODO: is it a problem to clone comments?
                    let comments = origin_comments.get_swc_comments().clone();
                    folders.push(Box::new(swc_preset_env::preset_env(
                        unresolved_mark,
                        Some(comments),
                        swc_preset_env::Config {
                            mode: Some(swc_preset_env::Mode::Entry),
                            targets: Some(targets::swc_preset_env_targets_from_map(
                                context.config.targets.clone(),
                            )),
                            ..Default::default()
                        },
                        Assumptions::default(),
                        &mut FeatureFlag::default(),
                    )));
                    folders.push(Box::new(decorators(decorators::Config {
                        legacy: true,
                        emit_metadata: false,
                        ..Default::default()
                    })));
                    // simplify, but keep top level dead code
                    // e.g. import x from 'foo'; but x is not used
                    // this must be kept for tree shaking to work
                    folders.push(Box::new(simplifier(
                        unresolved_mark,
                        SimpilifyConfig {
                            dce: dce::Config {
                                top_level: false,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    )));
                    // NOTICE: remove optimize_package_imports temporarily
                    // folders.push(Box::new(Optional {
                    //     enabled: should_optimize(file.path.to_str().unwrap(), context.clone()),
                    //     visitor: optimize_package_imports(
                    //         file.path.to_string_lossy().to_string(),
                    //         context.clone(),
                    //     ),
                    // }));

                    ast.transform(&mut visitors, &mut folders, file, true, context.clone())?;

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
                visitors.push(Box::new(CSSUrlReplacer {
                    path,
                    context: context.clone(),
                }));
                // same ability as postcss-flexbugs-fixes
                if context.config.flex_bugs {
                    visitors.push(Box::new(CSSFlexbugs {}));
                }
                if context.config.px2rem.is_some() {
                    let context = context.clone();
                    visitors.push(Box::new(Px2Rem {
                        path: file.path.to_string_lossy().to_string(),
                        context: context.clone(),
                        current_decl: None,
                        current_selector: None,
                    }));
                }
                // prefixer
                visitors.push(Box::new(swc_css_prefixer::prefixer(
                    swc_css_prefixer::options::Options {
                        env: Some(targets::swc_preset_env_targets_from_map(
                            context.config.targets.clone(),
                        )),
                    },
                )));
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
