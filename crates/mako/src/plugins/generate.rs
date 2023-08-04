use std::collections::HashMap;
use std::fs;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use pathdiff::diff_paths;
use swc_common::errors::HANDLER;
use swc_common::GLOBALS;
use swc_ecma_ast::Ident;
use swc_ecma_transforms::fixer;
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::ImportInterop;
use swc_ecma_visit::{VisitMut, VisitMutWith};
use swc_error_reporters::handler::try_with_handler;

use crate::ast::{js_ast_to_code, Ast};
use crate::compiler::Context;
use crate::config::Mode;
use crate::module::{ModuleAst, ModuleId};
use crate::plugin::Plugin;
use crate::transform_dep_replacer::DepReplacer;
use crate::transform_dynamic_import::DynamicImport;
use crate::transform_in_generate::transform_css;
use crate::transform_react::PrefixCode;
use crate::unused_statement_sweep::UnusedStatementSweep;

pub struct MinifishGenerator {}

impl MinifishGenerator {
    pub fn transform_all(&self, context: &Arc<Context>) -> Result<()> {
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        transform_modules(module_ids, context)?;
        Ok(())
    }

    pub fn write_to_dist<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(
        &self,
        filename: P,
        content: C,
        context: &Arc<Context>,
    ) {
        let to = context
            .config
            .output
            .path
            .join(filename)
            .with_extension("js");

        let parent = to.parent().unwrap();
        // TODO try tokio fs later
        create_dir_all(parent).unwrap();
        fs::write(to, content).unwrap();
    }
}

impl Plugin for MinifishGenerator {
    fn name(&self) -> &str {
        "minifish_generator"
    }

    fn generate(&self, context: &Arc<Context>) -> Result<Option<()>> {
        self.transform_all(context)?;

        let mg = context.module_graph.read().unwrap();

        let ids = mg.get_module_ids();

        let _output = context.config.output.path.canonicalize().unwrap();

        ids.iter().for_each(|id| {
            let module = mg.get_module(id).expect("module not exits");

            let info = module.info.as_ref().expect("module info missing");

            match &info.ast {
                ModuleAst::Script(js_ast) => {
                    if module.id.id.ends_with(".json") {
                        // nothing
                        // todo: generate resolved AJSON
                    } else {
                        let (code, _) = js_ast_to_code(&js_ast.ast, context, "a.js")
                            .unwrap_or(("".to_string(), "".to_string()));

                        let target = to_dist_path(&id.id, context);

                        self.write_to_dist(target, code, context);
                    }
                }
                ModuleAst::Css(_style) => {}
                ModuleAst::None => {
                    //  nothing
                }
            }
        });

        Ok(Some(()))
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let module_dist_path = to_dist_path(&module_id.id, context)
            .parent()
            .unwrap()
            .to_path_buf();

        let dep_map: HashMap<String, String> = deps
            .clone()
            .into_iter()
            // .map(|(id, dep)| (dep.source.clone(), id.generate(context)))
            .map(|(id, dep)| {
                let dep_dist_path = to_dist_path(&id.id, context);

                let rel_path = diff_paths(dep_dist_path, &module_dist_path)
                    .unwrap()
                    .with_extension("js");

                let replacement: String = {
                    let mut to_path = rel_path.to_str().unwrap().to_string();
                    if to_path.starts_with('.') {
                        to_path
                    } else {
                        to_path.insert_str(0, "./");
                        to_path
                    }
                };

                (dep.source.clone(), replacement)
            })
            .collect();
        let assets_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
            .collect();
        drop(module_graph);

        // let deps: Vec<(&ModuleId, &crate::module::Dependency)> =
        //     module_graph.get_dependencies(module_id);
        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;

        if let ModuleAst::Script(ast) = ast {
            transform_js_generate2(&module.id, context, ast, &dep_map, module.is_entry);
        }

        // 通过开关控制是否单独提取css文件
        if !context.config.extract_css {
            if let ModuleAst::Css(ast) = ast {
                let ast = transform_css(ast, &path, dep_map, assets_map, context);
                info.set_ast(ModuleAst::Script(ast));
            }
        }
    });
    Ok(())
}

pub fn transform_js_generate2(
    id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &HashMap<String, String>,
    _is_entry: bool,
) {
    let _is_dev = matches!(context.config.mode, Mode::Development);
    GLOBALS
        .set(&context.meta.script.globals, || {
            try_with_handler(
                context.meta.script.cm.clone(),
                Default::default(),
                |handler| {
                    HELPERS.set(&Helpers::new(true), || {
                        HANDLER.set(handler, || {
                            let _unresolved_mark = ast.unresolved_mark;
                            let top_level_mark = ast.top_level_mark;
                            // let (code, ..) = js_ast_to_code(&ast.ast, context, "foo").unwrap();
                            // print!("{}", code);

                            let mut v = GlobalThisPolyfill::new("globalThis".to_string());
                            ast.ast.visit_mut_with(&mut v);

                            if v.met_identifier {
                                let out = context.config.output.path.canonicalize().unwrap();

                                let virtual_path =
                                    out.join("_virtual/_minifish-polyfill-global.js");

                                let virtual_source = diff_paths(
                                    virtual_path,
                                    to_dist_path(&id.id, context).parent().unwrap(),
                                )
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string();

                                let mut prefix = PrefixCode {
                                    code: format!(
                                        r#"var globalThis = require("{}").__minifish_global__)"#,
                                        virtual_source
                                    ),
                                    context: context.clone(),
                                };

                                ast.ast.visit_mut_with(&mut prefix);
                            }

                            {
                                if context.config.minify
                                    && matches!(context.config.mode, Mode::Production)
                                {
                                    let comments =
                                        context.meta.script.output_comments.read().unwrap();
                                    let mut unused_statement_sweep =
                                        UnusedStatementSweep::new(id, &comments);
                                    ast.ast.visit_mut_with(&mut unused_statement_sweep);
                                }
                            }

                            let import_interop = ImportInterop::Swc;
                            // FIXME: 执行两轮 import_analyzer + inject_helpers，第一轮是为了 module_graph，第二轮是为了依赖替换
                            ast.ast
                                .visit_mut_with(&mut import_analyzer(import_interop, true));
                            // ast.ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

                            let mut dep_replacer = DepReplacer {
                                dep_map: dep_map.clone(),
                                context,
                            };
                            ast.ast.visit_mut_with(&mut dep_replacer);

                            let mut dynamic_import = DynamicImport { context };
                            ast.ast.visit_mut_with(&mut dynamic_import);

                            ast.ast.visit_mut_with(&mut hygiene_with_config(
                                swc_ecma_transforms::hygiene::Config {
                                    top_level_mark,
                                    ..Default::default()
                                },
                            ));
                            ast.ast.visit_mut_with(&mut fixer(Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            )));

                            Ok(())
                        })
                    })
                },
            )
        })
        .unwrap();
}

pub fn to_dist_path<P: AsRef<str>>(abs_path: P, context: &Arc<Context>) -> PathBuf {
    let src_root = context
        .root
        .clone()
        .join("src")
        .to_str()
        .unwrap()
        .to_string();
    let npm_root = context
        .root
        .clone()
        .join("node_modules")
        .to_str()
        .unwrap()
        .to_string();
    let output = context.config.output.path.clone().canonicalize().unwrap();

    let str = abs_path.as_ref();

    if str.contains(&src_root) {
        let relative_path = diff_paths(str, &src_root).unwrap();

        output.join(relative_path)
    } else if str.contains(&npm_root) {
        let relative_path = diff_paths(str, &context.root).unwrap();

        output.join(relative_path)
    } else {
        abs_path.as_ref().clone().into()
    }
}

struct GlobalThisPolyfill {
    ident_name: String,
    met_identifier: bool,
}

impl GlobalThisPolyfill {
    pub fn new(ident_name: String) -> Self {
        Self {
            met_identifier: false,
            ident_name,
        }
    }
}

impl VisitMut for GlobalThisPolyfill {
    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        // fn visit_ident(&mut self, ident: &Ident) {
        if ident.sym == self.ident_name {
            self.met_identifier = true;
        }
    }
}
