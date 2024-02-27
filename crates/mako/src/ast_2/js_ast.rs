use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_common::errors::HANDLER;
use mako_core::swc_common::util::take::Take;
use mako_core::swc_common::{FileName, Mark, GLOBALS};
use mako_core::swc_ecma_ast::{EsVersion, Module};
use mako_core::swc_ecma_codegen::text_writer::JsWriter;
use mako_core::swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use mako_core::swc_ecma_parser::error::SyntaxError;
use mako_core::swc_ecma_parser::lexer::Lexer;
use mako_core::swc_ecma_parser::{EsConfig, Parser, StringInput, Syntax, TsConfig};
use mako_core::swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use mako_core::swc_ecma_visit;
use mako_core::swc_ecma_visit::{VisitMutWith, VisitWith};
use swc_core::base::try_with_handler;
use swc_core::common::Spanned;

use crate::ast_2::file::File;
use crate::ast_2::{error, utils};
use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::module::Dependency;
use crate::sourcemap::build_source_map;
use crate::visitors::js_dep_analyzer::JSDepAnalyzer;
use std::fmt;

#[derive(Clone)]
pub struct JsAst {
    pub ast: Module,
    pub unresolved_mark: Mark,
    pub top_level_mark: Mark,
    path: String,
    context: Arc<Context>,
}

impl fmt::Debug for JsAst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JsAst")
    }
}

impl JsAst {
    pub fn new(file: &File, context: Arc<Context>) -> Result<Self> {
        let fm = context.meta.script.cm.new_source_file(
            FileName::Real(file.relative_path.to_path_buf()),
            file.get_content_raw(),
        );
        let comments = context.meta.script.origin_comments.read().unwrap();
        let extname = &file.extname;
        let syntax = if extname == "ts" || extname == "tsx" {
            Syntax::Typescript(TsConfig {
                tsx: extname == "tsx",
                decorators: true,
                ..Default::default()
            })
        } else {
            let jsx = extname == "jsx"
                // when use svg as svgr, it should come here and be treated as jsx
                || extname == "svg"
                || (extname == "js" && !file.is_under_node_modules);
            Syntax::Es(EsConfig {
                jsx,
                decorators: true,
                decorators_before_export: true,
                ..Default::default()
            })
        };
        let lexer = Lexer::new(
            syntax,
            EsVersion::Es2015,
            StringInput::from(&*fm),
            Some(comments.get_swc_comments()),
        );
        let mut parser = Parser::new_from(lexer);
        let ast = parser.parse_module();

        // handle ast errors
        let mut ast_errors = parser.take_errors();
        // ignore with syntax error in strict mode
        ast_errors.retain_mut(|error| !matches!(error.kind(), SyntaxError::WithInStrict));
        if ast.is_err() {
            ast_errors.push(ast.clone().unwrap_err());
        }
        if ast_errors.len() > 0 {
            let errors = ast_errors
                .iter()
                .map(|err| {
                    error::code_frame(
                        err.span(),
                        err.kind().msg().to_string().as_str(),
                        context.clone(),
                    )
                })
                .collect::<Vec<String>>();
            return Err(anyhow!(error::ParseError::JsParseError {
                messages: errors.join("\n")
            }));
        }
        let ast = ast./*safe*/unwrap();

        // top level mark and unresolved mark need to be persisted for transform usage
        GLOBALS.set(&context.meta.script.globals, || {
            let top_level_mark = Mark::new();
            let unresolved_mark = Mark::new();
            Ok(JsAst {
                ast,
                unresolved_mark,
                top_level_mark,
                path: file.relative_path.to_string_lossy().to_string(),
                context: context.clone(),
            })
        })
    }

    pub fn transform(
        &mut self,
        mut_visitors: &mut Vec<Box<dyn swc_ecma_visit::VisitMut>>,
        folders: Vec<Box<dyn swc_ecma_visit::Fold>>,
        should_inject_helpers: bool,
    ) -> Result<()>
     {
        let cm = self.context.meta.script.cm.clone();
        GLOBALS.set(&self.context.meta.script.globals, || {
            try_with_handler(cm, Default::default(), |handler| {
                HELPERS.set(&Helpers::new(true), || {
                    HANDLER.set(handler, || {
                        let ast = &mut self.ast;

                        // visitors
                        for visitor in mut_visitors {
                            ast.visit_mut_with(visitor.as_mut());
                        }

                        // folders
                        let body = ast.body.take();
                        let mut module = Module {
                            span: ast.span,
                            shebang: ast.shebang.clone(),
                            body,
                        };
                        for folder in folders.iter() {
                            module = folder.fold_module(module);
                        }
                        ast.body = module.body;

                        // FIXME: remove this, it's special logic
                        // inject helpers
                        // why need to handle cjs specially?
                        // because the ast is currently a module, not a program
                        // if not handled specially, the injected helpers will all be in esm format
                        // which is not as expected in the cjs scenario
                        // ref: https://github.com/umijs/mako/pull/831
                        if should_inject_helpers {
                            if utils::is_esm(ast) {
                                ast.visit_mut_with(&mut inject_helpers(self.unresolved_mark));
                            } else {
                                let body = ast.body.take();
                                let mut script_ast = swc_core::ecma::ast::Script {
                                    span: ast.span,
                                    shebang: ast.shebang.clone(),
                                    body: body
                                        .into_iter()
                                        .map(|i| i.clone().stmt().unwrap())
                                        .collect(),
                                };
                                script_ast
                                    .visit_mut_with(&mut inject_helpers(self.unresolved_mark));
                                ast.body = script_ast.body.into_iter().map(|i| i.into()).collect();
                            }
                        }

                        Ok(())
                    })
                })
            })
        })
    }

    pub fn analyze_deps(&self) -> Vec<Dependency> {
        let mut visitor = JSDepAnalyzer::new(self.unresolved_mark);
        GLOBALS.set(&self.context.meta.script.globals, || {
            self.ast.visit_with(&mut visitor);
            visitor.dependencies
        })
    }

    pub fn generate(&self) -> Result<JSAstGenerated> {
        let context = self.context.clone();
        let mut buf = vec![];
        let mut source_map_buf = vec![];
        let cm = context.meta.script.cm.clone();
        {
            let comments = context.meta.script.origin_comments.read().unwrap();
            let swc_comments = comments.get_swc_comments();
            let is_prod = matches!(context.config.mode, Mode::Production);
            let minify = context.config.minify && is_prod;
            let mut emitter = Emitter {
                cfg: JsCodegenConfig::default()
                    .with_minify(minify)
                    .with_target(context.config.output.es_version)
                    .with_ascii_only(context.config.output.ascii_only)
                    .with_omit_last_semi(true),
                cm: cm.clone(),
                comments: if minify { None } else { Some(swc_comments) },
                wr: Box::new(JsWriter::new(
                    cm.clone(),
                    "\n",
                    &mut buf,
                    Some(&mut source_map_buf),
                )),
            };
            emitter.emit_module(&self.ast).map_err(|err| {
                anyhow!(error::GenerateError::JsGenerateError {
                    message: err.to_string()
                })
            })?;
        }

        let sourcemap = match context.config.devtool {
            Some(DevtoolConfig::SourceMap | DevtoolConfig::InlineSourceMap) => {
                let src_buf = build_source_map(&source_map_buf, &cm);
                String::from_utf8(src_buf).unwrap()
            }
            None => "".to_string(),
        };
        if matches!(context.config.devtool, Some(DevtoolConfig::SourceMap)) {
            let filename = &self.path;
            buf.append(
                &mut format!("\n//# sourceMappingURL={filename}.map")
                    .as_bytes()
                    .to_vec(),
            );
        } else if matches!(context.config.devtool, Some(DevtoolConfig::InlineSourceMap)) {
            buf.append(
                &mut format!(
                    "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                    utils::base64_encode(&sourcemap)
                )
                .as_bytes()
                .to_vec(),
            );
        }

        let code = String::from_utf8(buf)?;
        Ok(JSAstGenerated { code, sourcemap })
    }
}

pub struct JSAstGenerated {
    pub code: String,
    pub sourcemap: String,
}
