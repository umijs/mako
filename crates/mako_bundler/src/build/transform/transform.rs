use std::sync::Arc;
use swc_common::sync::Lrc;
use swc_common::{
    comments::{NoopComments, SingleThreadedComments},
    Globals, Mark, SourceMap, GLOBALS,
};
use swc_ecma_ast::Module;
use swc_ecma_transforms::helpers::inject_helpers;
use swc_ecma_transforms::hygiene::{hygiene_with_config, Config as HygieneConfig};
use swc_ecma_transforms::{
    feature::FeatureFlag,
    fixer,
    helpers::{Helpers, HELPERS},
    modules::{
        common_js,
        import_analysis::import_analyzer,
        util::{Config, ImportInterop},
    },
    react::{react, Options},
    resolver,
    typescript::strip_with_jsx,
};
use swc_ecma_visit::VisitMutWith;

use crate::context::Context;
use crate::module::ModuleAst;

pub struct TransformParam<'a> {
    pub path: &'a str,
    pub ast: &'a ModuleAst,
    pub cm: &'a Lrc<SourceMap>,
}

pub struct TransformResult {
    pub ast: Module,
}

pub fn transform(transform_param: &TransformParam, _context: &Arc<Context>) -> TransformResult {
    let globals = Globals::default();

    let module_ast = if let ModuleAst::Script(ast) = transform_param.ast {
        ast
    } else {
        panic!("not support module")
    };

    let mut ast = module_ast.clone();
    let cm = transform_param.cm.clone();
    GLOBALS.set(&globals, || {
        let helpers = Helpers::new(true);
        HELPERS.set(&helpers, || {
            let top_level_mark = Mark::new();
            let unresolved_mark = Mark::new();
            let features = FeatureFlag::empty();

            let import_interop = ImportInterop::Swc;
            ast.visit_mut_with(&mut react(
                cm.clone(),
                Some(NoopComments),
                Options {
                    import_source: Some("react".to_string()),
                    pragma: Some("React.createElement".into()),
                    pragma_frag: Some("React.Fragment".into()),
                    ..Default::default()
                },
                top_level_mark,
                unresolved_mark,
            ));

            ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
            ast.visit_mut_with(&mut import_analyzer(import_interop, true));
            ast.visit_mut_with(&mut inject_helpers(unresolved_mark));

            ast.visit_mut_with(&mut common_js::<SingleThreadedComments>(
                unresolved_mark,
                Config {
                    import_interop: Some(import_interop),
                    // NOTE: 这里后面要调整为注入自定义require
                    // ignore_dynamic: true,
                    preserve_import_meta: true,
                    ..Default::default()
                },
                features,
                None,
            ));

            ast.visit_mut_with(&mut strip_with_jsx(
                cm.clone(),
                Default::default(),
                NoopComments,
                top_level_mark,
            ));

            ast.visit_mut_with(&mut hygiene_with_config(HygieneConfig {
                top_level_mark,
                ..Default::default()
            }));
            ast.visit_mut_with(&mut fixer(None));
        });
    });

    TransformResult { ast }
}
