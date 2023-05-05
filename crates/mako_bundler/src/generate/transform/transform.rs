use std::collections::HashMap;

use swc_common::collections::AHashMap;
use swc_common::sync::Lrc;
use swc_common::DUMMY_SP;
use swc_common::{Globals, SourceMap, GLOBALS};
use swc_css_codegen::{
    writer::basic::{BasicCssWriter, BasicCssWriterConfig},
    CodeGenerator, CodegenConfig, Emit,
};
use swc_css_visit::VisitMutWith as CssVisitMutWith;
use swc_ecma_ast::{Expr, Lit, Str};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_visit::VisitMutWith;

use crate::context::Context;
use crate::module::ModuleAst;

use super::dep_replacer::DepReplacer;
use super::env_replacer::EnvReplacer;

pub struct TransformParam<'a> {
    pub ast: &'a ModuleAst,
    pub cm: &'a Lrc<SourceMap>,
    pub dep_map: HashMap<String, String>,
    pub env_map: HashMap<String, String>,
}

pub struct TransformResult {
    pub ast: ModuleAst,
    pub code: String,
}

pub fn transform(transform_param: &TransformParam, _context: &Context) -> TransformResult {
    if let ModuleAst::Script(ast) = transform_param.ast {
        let mut ast = ast.clone();

        let globals = Globals::default();
        let mut env_map = AHashMap::default();
        transform_param
            .env_map
            .clone()
            .into_iter()
            .for_each(|(k, v)| {
                env_map.insert(
                    k.into(),
                    Expr::Lit(Lit::Str(Str {
                        span: DUMMY_SP,
                        raw: None,
                        value: v.into(),
                    })),
                );
            });

        let cm = transform_param.cm.clone();
        GLOBALS.set(&globals, || {
            let helpers = Helpers::new(true);
            HELPERS.set(&helpers, || {
                let mut dep_replacer = DepReplacer {
                    dep_map: transform_param.dep_map.clone(),
                };
                ast.visit_mut_with(&mut dep_replacer);

                let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
                ast.visit_mut_with(&mut env_replacer);
            });
        });

        // ast to code
        let mut buf = Vec::new();
        {
            let mut emitter = Emitter {
                cfg: Default::default(),
                cm: cm.clone(),
                comments: None,
                wr: Box::new(JsWriter::new(cm, "\n", &mut buf, None)),
            };
            emitter.emit_module(&ast).unwrap();
        }
        let code = String::from_utf8(buf).unwrap();
        // println!("code: {}", code);

        TransformResult {
            ast: ModuleAst::Script(ast),
            code,
        }
    } else if let ModuleAst::Css(stylesheet) = transform_param.ast {
        let mut stylesheet = stylesheet.clone();

        let globals = Globals::default();
        GLOBALS.set(&globals, || {
            let helpers = Helpers::new(true);
            HELPERS.set(&helpers, || {
                let mut dep_replacer = DepReplacer {
                    dep_map: transform_param.dep_map.clone(),
                };
                stylesheet.visit_mut_with(&mut dep_replacer);
            });
        });

        let mut css_code = String::new();
        let css_writer = BasicCssWriter::new(&mut css_code, None, BasicCssWriterConfig::default());
        let mut gen = CodeGenerator::new(css_writer, CodegenConfig::default());

        gen.emit(&stylesheet).unwrap();

        let mut code_vec: Vec<String> = vec![];

        for value in transform_param.dep_map.values() {
            code_vec.push(format!("require(\"{}\");", value));
        }

        code_vec.push(format!(
            r#"
const cssCode = `{}`;
const style = document.createElement('style');
style.innerHTML = cssCode;
document.head.appendChild(style);
"#,
            css_code
        ));

        let code = code_vec.join("\n");

        TransformResult {
            ast: ModuleAst::Css(stylesheet),
            code,
        }
    } else {
        panic!("not support module")
    }
}
