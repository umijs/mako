use std::collections::{HashMap, HashSet};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

use mako_core::anyhow::Result;
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::GLOBALS;
use mako_core::swc_css_ast;
use mako_core::swc_css_visit::VisitMutWith as CSSVisitMutWith;
use mako_core::swc_ecma_transforms::feature::FeatureFlag;
use mako_core::swc_ecma_transforms::fixer::fixer;
use mako_core::swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use mako_core::swc_ecma_transforms::hygiene;
use mako_core::swc_ecma_transforms::hygiene::hygiene_with_config;
use mako_core::swc_ecma_transforms_modules::common_js;
use mako_core::swc_ecma_transforms_modules::import_analysis::import_analyzer;
use mako_core::swc_ecma_transforms_modules::util::{Config, ImportInterop};
use mako_core::swc_ecma_visit::VisitMutWith;
use mako_core::swc_error_reporters::handler::try_with_handler;
use mako_core::tracing::debug;

use crate::ast_2::js_ast::JsAst;
use crate::compiler::{Compiler, Context};
use crate::config::OutputMode;
use crate::module::{Dependency, ModuleAst, ModuleId, ResolveType};
use crate::swc_helpers::SwcHelpers;
use crate::thread_pool;
use crate::transformers::transform_async_module::AsyncModule;
use crate::transformers::transform_css_handler::CssHandler;
use crate::transformers::transform_dep_replacer::{DepReplacer, DependenciesToReplace};
use crate::transformers::transform_dynamic_import::DynamicImport;
use crate::transformers::transform_mako_require::MakoRequire;
use crate::transformers::transform_meta_url_replacer::MetaUrlReplacer;
use crate::transformers::transform_optimize_define_utils::OptimizeDefineUtils;

impl Compiler {
    pub fn transform_all(&self) -> Result<()> {
        let context = &self.context;
        let t = Instant::now();
        let module_graph = context.module_graph.read().unwrap();
        // Reversed after topo sorting, in order to better handle async module
        let (mut module_ids, _) = module_graph.toposort();
        module_ids.reverse();
        drop(module_graph);
        debug!(">> toposort & reverse in {}ms", t.elapsed().as_millis());
        transform_modules(module_ids, context)?;
        Ok(())
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    let t = Instant::now();
    let async_deps_by_module_id = mark_async(&module_ids, context);
    debug!(">> mark async in {}ms", t.elapsed().as_millis());
    let t = Instant::now();
    transform_modules_in_thread(&module_ids, context, async_deps_by_module_id)?;
    debug!(">> transform modules in {}ms", t.elapsed().as_millis());
    Ok(())
}

fn mark_async(
    module_ids: &[ModuleId],
    context: &Arc<Context>,
) -> HashMap<ModuleId, Vec<Dependency>> {
    mako_core::mako_profile_function!();
    let mut async_deps_by_module_id = HashMap::new();
    let mut module_graph = context.module_graph.write().unwrap();
    // TODO: 考虑成环的场景
    module_ids.iter().for_each(|module_id| {
        let deps = module_graph.get_dependencies_info(module_id);
        let async_deps: Vec<Dependency> = deps
            .into_iter()
            .filter(|(_, dep, is_async)| {
                matches!(dep.resolve_type, ResolveType::Import | ResolveType::Require) && *is_async
            })
            .map(|(_, dep, _)| dep.clone())
            .collect();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        // a module with async deps need to be polluted into async module
        if !info.is_async && !async_deps.is_empty() {
            info.is_async = true;
        }
        async_deps_by_module_id.insert(module_id.clone(), async_deps);
    });
    async_deps_by_module_id
}

pub fn transform_modules_in_thread(
    module_ids: &Vec<ModuleId>,
    context: &Arc<Context>,
    async_deps_by_module_id: HashMap<ModuleId, Vec<Dependency>>,
) -> Result<()> {
    mako_core::mako_profile_function!();

    let (rs, rr) = channel::<Result<(ModuleId, ModuleAst, Option<HashSet<String>>)>>();

    for module_id in module_ids {
        let context = context.clone();
        let rs = rs.clone();
        let module_id = module_id.clone();
        let async_deps = async_deps_by_module_id.get(&module_id).unwrap().clone();
        thread_pool::spawn(move || {
            let module_graph = context.module_graph.read().unwrap();
            let deps = module_graph.get_dependencies(&module_id);
            let mut resolved_deps: HashMap<String, (String, String)> = deps
                .into_iter()
                .map(|(id, dep)| {
                    (
                        dep.source.clone(),
                        (
                            if dep.resolve_type == ResolveType::Worker {
                                let chunk_id = id.generate(&context);
                                let chunk_graph = context.chunk_graph.read().unwrap();
                                chunk_graph.chunk(&chunk_id.into()).unwrap().filename()
                            } else {
                                id.generate(&context)
                            },
                            id.id.clone(),
                        ),
                    )
                })
                .collect();
            insert_swc_helper_replace(&mut resolved_deps, &context);
            let module = module_graph.get_module(&module_id).unwrap();
            let info = module.info.as_ref().unwrap();
            let ast = info.ast.clone();
            let deps_to_replace = DependenciesToReplace {
                resolved: resolved_deps,
                missing: info.deps.missing_deps.clone(),
            };
            if let ModuleAst::Script(mut ast) = ast {
                let ret = transform_js_generate(TransformJsParam {
                    module_id: &module.id,
                    context: &context,
                    ast: &mut ast,
                    dep_map: &deps_to_replace,
                    async_deps: &async_deps,
                    wrap_async: info.is_async && info.external.is_none(),
                    top_level_await: info.top_level_await,
                });
                let message = match ret {
                    Ok(_) => {
                        let swc_helpers = if context.args.watch {
                            None
                        } else {
                            Some(SwcHelpers::get_swc_helpers(&ast.ast, &context))
                        };
                        Ok((module_id, ModuleAst::Script(ast), swc_helpers))
                    }
                    Err(e) => Err(e),
                };
                rs.send(message).unwrap();
            }
        });
    }
    drop(rs);

    let mut transform_map: HashMap<ModuleId, (ModuleAst, Option<HashSet<String>>)> = HashMap::new();
    for r in rr {
        let (module_id, ast, swc_helpers) = r?;
        transform_map.insert(module_id, (ast, swc_helpers));
    }

    let mut module_graph = context.module_graph.write().unwrap();
    for (module_id, (ast, swc_helpers)) in transform_map {
        let module = module_graph.get_module_mut(&module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        info.ast = ast;
        if let Some(swc_helpers) = swc_helpers {
            context.swc_helpers.lock().unwrap().extends(swc_helpers);
        }
    }

    Ok(())
}

fn insert_swc_helper_replace(map: &mut HashMap<String, (String, String)>, context: &Arc<Context>) {
    let helpers = vec![
        "@swc/helpers/_/_interop_require_default",
        "@swc/helpers/_/_interop_require_wildcard",
        "@swc/helpers/_/_export_star",
    ];

    helpers.into_iter().for_each(|h| {
        let m_id: ModuleId = h.to_string().into();
        map.insert(m_id.id.clone(), (m_id.generate(context), h.to_string()));
    });
}

pub struct TransformJsParam<'a> {
    pub module_id: &'a ModuleId,
    pub context: &'a Arc<Context>,
    pub ast: &'a mut JsAst,
    pub dep_map: &'a DependenciesToReplace,
    pub async_deps: &'a Vec<Dependency>,
    pub wrap_async: bool,
    pub top_level_await: bool,
}

pub fn transform_js_generate(transform_js_param: TransformJsParam) -> Result<()> {
    mako_core::mako_profile_function!();
    let TransformJsParam {
        module_id,
        context,
        ast,
        dep_map,
        async_deps,
        wrap_async,
        top_level_await,
    } = transform_js_param;
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(
            context.meta.script.cm.clone(),
            Default::default(),
            |handler| {
                HELPERS.set(&Helpers::new(true), || {
                    HANDLER.set(handler, || {
                        let unresolved_mark = ast.unresolved_mark;
                        let top_level_mark = ast.top_level_mark;

                        let import_interop = ImportInterop::Swc;
                        ast.ast
                            .visit_mut_with(&mut import_analyzer(import_interop, true));
                        ast.ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                        ast.ast.visit_mut_with(&mut common_js(
                            unresolved_mark,
                            Config {
                                import_interop: Some(import_interop),
                                // NOTE: 这里后面要调整为注入自定义require
                                ignore_dynamic: true,
                                preserve_import_meta: true,
                                // TODO: 在 esm 时设置为 false
                                allow_top_level_this: true,
                                strict_mode: false,
                                ..Default::default()
                            },
                            FeatureFlag::empty(),
                            Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            ),
                        ));

                        ast.ast.visit_mut_with(&mut OptimizeDefineUtils {
                            top_level_mark,
                            unresolved_mark,
                        });

                        // transform async module
                        if wrap_async {
                            let mut async_module = AsyncModule {
                                async_deps,
                                async_deps_idents: Vec::new(),
                                last_dep_pos: 0,
                                top_level_await,
                                context,
                                unresolved_mark,
                            };
                            ast.ast.visit_mut_with(&mut async_module);
                        }

                        let mut dep_replacer = DepReplacer {
                            module_id,
                            to_replace: dep_map,
                            context,
                            unresolved_mark,
                            top_level_mark,
                        };
                        ast.ast.visit_mut_with(&mut dep_replacer);

                        let mut meta_url_replacer = MetaUrlReplacer {};
                        ast.ast.visit_mut_with(&mut meta_url_replacer);

                        let mut dynamic_import = DynamicImport { context };
                        ast.ast.visit_mut_with(&mut dynamic_import);

                        // replace require to __mako_require__ for bundle mode
                        if matches!(context.config.output.mode, OutputMode::Bundle) {
                            let mut mako_require = MakoRequire::new(context, unresolved_mark);
                            ast.ast.visit_mut_with(&mut mako_require);
                        }

                        ast.ast
                            .visit_mut_with(&mut hygiene_with_config(hygiene::Config {
                                top_level_mark,
                                ..Default::default()
                            }));

                        let origin_comments = context.meta.script.origin_comments.read().unwrap();
                        let swc_comments = origin_comments.get_swc_comments();
                        ast.ast.visit_mut_with(&mut fixer(Some(swc_comments)));

                        Ok(())
                    })
                })
            },
        )
    })
}

pub fn transform_css_generate(ast: &mut swc_css_ast::Stylesheet, _context: &Arc<Context>) {
    mako_core::mako_profile_function!();
    // replace deps
    let mut css_handler = CssHandler {};
    ast.visit_mut_with(&mut css_handler);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::transform_css_generate;
    use crate::ast::{build_css_ast, css_ast_to_code};

    #[test]
    fn test_transform_css_import() {
        let code = r#"
@import "./bar.css";
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#".foo {
  color: red;
}
/*# sourceMappingURL=test.css.map*/"#
                .trim()
        );
    }

    #[test]
    fn test_transform_css_import_hoist() {
        let code = r#"
@import "https://example.com/first.css";
.foo { color: red; }
@import "https://example.com/foo.css";
.bar { color: blue; }
@import "https://example.com/bar.css";
.other { color: green; }
@media print {}
@import "https://example.com/other.css";
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"@import "https://example.com/first.css";
@import "https://example.com/foo.css";
@import "https://example.com/bar.css";
@import "https://example.com/other.css";
.foo {
  color: red;
}
.bar {
  color: blue;
}
.other {
  color: green;
}
@media print {}
/*# sourceMappingURL=test.css.map*/"#
                .trim()
        );
    }

    fn transform_css_code(content: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let context = Arc::new(Default::default());
        let mut ast = build_css_ast(path, content, &context, false).unwrap();
        transform_css_generate(&mut ast, &context);
        let (code, _sourcemap) = css_ast_to_code(&ast, &context, "test.css");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
