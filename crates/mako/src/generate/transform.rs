use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Error, Result};
use regex::Regex;
use swc_core::base::try_with_handler;
use swc_core::common::errors::HANDLER;
use swc_core::common::GLOBALS;
use swc_core::css::ast;
use swc_core::css::visit::VisitMutWith as CSSVisitMutWith;
use swc_core::ecma::transforms::base::fixer::fixer;
use swc_core::ecma::transforms::base::helpers::{inject_helpers, Helpers, HELPERS};
use swc_core::ecma::transforms::base::hygiene;
use swc_core::ecma::transforms::base::hygiene::hygiene_with_config;
use swc_core::ecma::transforms::module::import_analysis::import_analyzer;
use swc_core::ecma::transforms::module::util::ImportInterop;
use swc_core::ecma::visit::VisitMutWith;
use tracing::debug;

use crate::ast::js_ast::JsAst;
use crate::compiler::{Compiler, Context};
use crate::module::{generate_module_id, Dependency, ModuleAst, ModuleId, ModuleType, ResolveType};
use crate::share::helpers::SWC_HELPERS;
use crate::utils::thread_pool;
use crate::visitors::async_module::{mark_async, AsyncModule};
use crate::visitors::common_js::common_js;
use crate::visitors::css_imports::CSSImports;
use crate::visitors::dep_replacer::{DepReplacer, DependenciesToReplace, ResolvedReplaceInfo};
use crate::visitors::dynamic_import::DynamicImport;
use crate::visitors::mako_require::MakoRequire;
use crate::visitors::meta_url_replacer::MetaUrlReplacer;
use crate::visitors::optimize_define_utils::OptimizeDefineUtils;

impl Compiler {
    pub fn transform_all(&self, async_deps_map: HashMap<ModuleId, Vec<Dependency>>) -> Result<()> {
        let t = Instant::now();
        let context = &self.context;
        let module_ids = {
            let module_graph = context.module_graph.read().unwrap();
            module_graph
                .modules()
                .into_iter()
                .filter(|m| m.get_module_type() != ModuleType::PlaceHolder)
                .map(|m| m.id.clone())
                .collect::<Vec<_>>()
        };

        transform_modules_in_thread(&module_ids, context, async_deps_map)?;
        debug!(">> transform modules in {}ms", t.elapsed().as_millis());
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

pub fn transform_modules_in_thread(
    module_ids: &Vec<ModuleId>,
    context: &Arc<Context>,
    async_deps_by_module_id: HashMap<ModuleId, Vec<Dependency>>,
) -> Result<()> {
    crate::mako_profile_function!();

    let (rs, rr) = channel::<Result<(ModuleId, ModuleAst)>>();

    for module_id in module_ids {
        let context = context.clone();
        let rs = rs.clone();
        let module_id = module_id.clone();
        let async_deps = async_deps_by_module_id
            .get(&module_id)
            .cloned()
            .unwrap_or(vec![]);

        thread_pool::spawn(move || {
            let module_graph = context.module_graph.read().unwrap();
            let deps = module_graph.get_dependencies(&module_id);
            let mut resolved_deps: HashMap<String, ResolvedReplaceInfo> = Default::default();

            deps.into_iter().for_each(|(id, dep)| {
                let replace_info = match &dep.resolve_type {
                    ResolveType::Worker(import_options) => {
                        let chunk_id = match import_options.get_chunk_name() {
                            Some(chunk_name) => generate_module_id(chunk_name, &context),
                            None => id.generate(&context),
                        };
                        let chunk_graph = context.chunk_graph.read().unwrap();
                        let chunk_name = chunk_graph.chunk(&chunk_id.into()).unwrap().filename();

                        ResolvedReplaceInfo {
                            chunk_id: None,
                            to_replace_source: chunk_name,
                            resolved_module_id: id.clone(),
                        }
                    }
                    ResolveType::DynamicImport(import_options) => {
                        let chunk_id = Some(match import_options.get_chunk_name() {
                            Some(chunk_name) => generate_module_id(chunk_name, &context),
                            None => id.generate(&context),
                        });

                        ResolvedReplaceInfo {
                            chunk_id,
                            to_replace_source: id.generate(&context),
                            resolved_module_id: id.clone(),
                        }
                    }
                    _ => ResolvedReplaceInfo {
                        chunk_id: None,
                        to_replace_source: id.generate(&context),
                        resolved_module_id: id.clone(),
                    },
                };

                resolved_deps
                    .entry(dep.source.clone())
                    .and_modify(|info: &mut ResolvedReplaceInfo| {
                        match (&replace_info.chunk_id, &info.chunk_id) {
                            (None, _) => {}
                            (Some(id), _) => info.chunk_id = Some(id.clone()),
                        }
                    })
                    .or_insert(replace_info);
            });
            insert_swc_helper_replace(&mut resolved_deps, &context);
            let module = module_graph.get_module(&module_id).unwrap();
            let info = module.info.as_ref().unwrap();
            let ast = info.ast.clone();
            let deps_to_replace = DependenciesToReplace {
                resolved: resolved_deps,
                missing: info.deps.missing_deps.clone(),
            };
            if let ModuleAst::Script(mut ast) = ast {
                let wrap_async = info.is_async && info.external.is_none();

                let ret = transform_js_generate(TransformJsParam {
                    module_id: &module.id,
                    context: &context,
                    ast: &mut ast,
                    dep_map: &deps_to_replace,
                    async_deps: &async_deps,
                    wrap_async,
                    top_level_await: info.top_level_await,
                });
                let message = match ret {
                    Ok(_) => Ok((module_id, ModuleAst::Script(ast))),
                    Err(e) => Err(e),
                };
                rs.send(message).unwrap();
            }
        });
    }
    drop(rs);

    let mut transform_map: HashMap<ModuleId, ModuleAst> = HashMap::new();
    for r in rr {
        let (module_id, ast) = r?;
        transform_map.insert(module_id, ast);
    }

    let mut module_graph = context.module_graph.write().unwrap();
    for (module_id, ast) in transform_map {
        let module = module_graph.get_module_mut(&module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        info.ast = ast;
    }

    Ok(())
}

fn insert_swc_helper_replace(
    map: &mut HashMap<String, ResolvedReplaceInfo>,
    context: &Arc<Context>,
) {
    SWC_HELPERS.into_iter().for_each(|h| {
        let m_id: ModuleId = h.to_string().into();
        map.insert(
            m_id.id.clone(),
            ResolvedReplaceInfo {
                chunk_id: None,
                to_replace_source: m_id.generate(context),
                resolved_module_id: m_id,
            },
        );
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
    crate::mako_profile_function!();
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
                            context.clone(),
                            unresolved_mark,
                            import_interop,
                        ));

                        ast.ast.visit_mut_with(&mut OptimizeDefineUtils {
                            top_level_mark,
                            unresolved_mark,
                        });

                        // transform async module
                        if wrap_async {
                            let mut async_module =
                                AsyncModule::new(async_deps, unresolved_mark, top_level_await);
                            ast.ast.visit_mut_with(&mut async_module);
                        }

                        let mut dep_replacer = DepReplacer {
                            module_id,
                            to_replace: dep_map,
                            context,
                            unresolved_mark,
                        };
                        ast.ast.visit_mut_with(&mut dep_replacer);

                        let mut meta_url_replacer = MetaUrlReplacer {};
                        ast.ast.visit_mut_with(&mut meta_url_replacer);

                        let mut dynamic_import = DynamicImport::new(context.clone(), dep_map);
                        ast.ast.visit_mut_with(&mut dynamic_import);

                        // replace require to __mako_require__
                        let ignores = context
                            .config
                            .ignores
                            .iter()
                            .map(|ignore| Regex::new(ignore).map_err(Error::new))
                            .collect::<Result<Vec<Regex>>>()?;
                        let mut mako_require = MakoRequire {
                            ignores,
                            unresolved_mark,
                            context: context.clone(),
                        };
                        ast.ast.visit_mut_with(&mut mako_require);

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

pub fn transform_css_generate(ast: &mut ast::Stylesheet, _context: &Arc<Context>) {
    crate::mako_profile_function!();
    // replace deps
    let mut css_handler = CSSImports {};
    ast.visit_mut_with(&mut css_handler);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::transform_css_generate;
    use crate::ast::css_ast::CssAst;
    use crate::compiler::Context;

    #[test]
    fn test_transform_css_import() {
        let code = r#"
@import "./bar.css";
.foo { color: red; }
        "#
        .trim();
        let code = transform_css_code(code, None);
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
        let code = transform_css_code(code, None);
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

    fn transform_css_code(content: &str, path: Option<&str>) -> String {
        let path = path.unwrap_or("test.css");
        let context: Arc<Context> = Arc::new(Default::default());
        let mut ast = CssAst::build(path, content, context.clone(), false).unwrap();
        transform_css_generate(&mut ast.ast, &context);
        let code = ast.generate(context.clone()).unwrap().code;
        println!("{}", code);
        code
    }
}
