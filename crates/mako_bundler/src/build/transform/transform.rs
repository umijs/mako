use std::collections::HashMap;

use swc_common::sync::Lrc;
use swc_common::{
    comments::{NoopComments, SingleThreadedComments},
    Globals, Mark, SourceMap, GLOBALS,
};
use swc_ecma_ast::Module;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_transforms::{
    feature::FeatureFlag,
    helpers::{Helpers, HELPERS},
    modules::{
        common_js,
        import_analysis::import_analyzer,
        util::{Config, ImportInterop},
    },
    react::{react, Options},
    typescript::strip_with_jsx,
};
use swc_ecma_visit::VisitMutWith;

use crate::context::Context;

use super::dep_replacer::DepReplacer;

pub struct TransformParam<'a> {
    pub path: &'a str,
    pub ast: Module,
    pub cm: Lrc<SourceMap>,
    pub dep_map: HashMap<String, String>,
}

pub struct TransformResult {
    pub ast: Module,
    pub code: String,
}

pub fn transform(transform_param: &TransformParam, _context: &Context) -> TransformResult {
    let globals = Globals::default();
    let mut ast = transform_param.ast.clone();
    let cm = transform_param.cm.clone();
    GLOBALS.set(&globals, || {
        let helpers = Helpers::new(true);
        HELPERS.set(&helpers, || {
            let top_level_mark = Mark::new();
            let unresolved_mark = Mark::new();
            let features = FeatureFlag::empty();
            ast.visit_mut_with(&mut import_analyzer(ImportInterop::Swc, true));
            ast.visit_mut_with(&mut common_js::<SingleThreadedComments>(
                unresolved_mark,
                Config {
                    import_interop: Some(ImportInterop::None),
                    ignore_dynamic: true,
                    ..Default::default()
                },
                features,
                None,
            ));
            ast.visit_mut_with(&mut react(
                cm.clone(),
                Some(NoopComments),
                Options {
                    ..Default::default()
                },
                top_level_mark,
            ));
            ast.visit_mut_with(&mut strip_with_jsx(
                cm.clone(),
                Default::default(),
                NoopComments,
                top_level_mark,
            ));
            let mut dep_replacer = DepReplacer {
                dep_map: transform_param.dep_map.clone(),
            };
            ast.visit_mut_with(&mut dep_replacer);
        });
    });

    // ast to code
    let mut buf = Vec::new();
    {
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None)),
        };
        emitter.emit_module(&ast).unwrap();
    }
    let code = String::from_utf8(buf).unwrap();
    // println!("code: {}", code);

    TransformResult { ast, code }
}
