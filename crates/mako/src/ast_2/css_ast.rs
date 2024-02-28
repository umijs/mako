use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use mako_core::swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use mako_core::swc_css_visit::{VisitMutWith, VisitWith};
use mako_core::swc_ecma_parser::StringInput;
use mako_core::{swc_css_parser, swc_css_visit};
use swc_core::common::FileName;

use crate::ast_2::file::File;
use crate::ast_2::{error, utils};
use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::module::Dependency;
use crate::sourcemap::build_source_map;
use crate::visitors::css_dep_analyzer::CSSDepAnalyzer;
use std::fmt;

#[derive(Clone)]
pub struct CssAst {
    pub ast: Stylesheet,
    pub path: String,
    context: Arc<Context>,
}

impl fmt::Debug for CssAst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CssAst")
    }
}

impl CssAst {
    pub fn new(file: &File, context: Arc<Context>) -> Result<Self> {
        let fm = context.meta.css.cm.new_source_file(
            FileName::Real(file.relative_path.clone()),
            file.get_content_raw(),
        );
        let config = swc_css_parser::parser::ParserConfig {
            css_modules: file.is_css_modules,
            legacy_ie: true,
            ..Default::default()
        };
        let lexer = swc_css_parser::lexer::Lexer::new(StringInput::from(&*fm), config);
        let mut parser = swc_css_parser::parser::Parser::new(lexer, config);
        let parse_result = parser.parse_all();
        let mut ast_errors = parser.take_errors();
        if parse_result.is_err() {
            ast_errors.push(parse_result.clone().unwrap_err());
        };
        if ast_errors.len() > 0 && !file.is_under_node_modules {
            let errors = ast_errors
                .iter()
                .map(|err| {
                    error::code_frame(
                        (*err.clone().into_inner()).0,
                        err.message().to_string().as_str(),
                        context.clone(),
                    )
                })
                .collect::<Vec<String>>();
            return Err(anyhow!(error::ParseError::CSSParseError {
                messages: errors.join("\n")
            }));
        }
        let ast = parse_result./*safe*/unwrap();
        Ok(Self {
            ast,
            path: file.relative_path.to_string_lossy().to_string(),
            context: context.clone(),
        })
    }

    pub fn analyze_deps(&self) -> Vec<Dependency> {
        let mut visitor = CSSDepAnalyzer::new();
        self.ast.visit_with(&mut visitor);
        visitor.dependencies
    }

    pub fn transform(
        &mut self,
        mut_visitors: &mut Vec<Box<dyn swc_css_visit::VisitMut>>,
    ) -> Result<()> {
        let ast = &mut self.ast;
        for visitor in mut_visitors {
            ast.visit_mut_with(visitor);
        }
        Ok(())
    }

    pub fn generate(&self) -> Result<CSSAstGenerated> {
        let context = self.context.clone();
        let mut code = String::new();
        let mut source_map = Vec::new();
        let writer = BasicCssWriter::new(
            &mut code,
            Some(&mut source_map),
            BasicCssWriterConfig::default(),
        );
        let mut gen = CodeGenerator::new(
            writer,
            CodegenConfig {
                minify: context.config.minify && matches!(context.config.mode, Mode::Production),
            },
        );
        gen.emit(&self.ast).map_err(|err| {
            anyhow!(error::GenerateError::CSSGenerateError {
                message: err.to_string()
            })
        })?;

        let buf = build_source_map(&source_map, &context.meta.css.cm);
        let sourcemap = String::from_utf8(buf).unwrap();
        if matches!(context.config.devtool, Some(DevtoolConfig::SourceMap)) {
            let filename = &self.path;
            code.push_str(format!("\n/*# sourceMappingURL={filename}.map*/").as_str());
        } else if matches!(context.config.devtool, Some(DevtoolConfig::InlineSourceMap)) {
            code.push_str(
                format!(
                    "\n/*# sourceMappingURL=data:application/json;charset=utf-8;base64,{}*/",
                    utils::base64_encode(&sourcemap)
                )
                .as_str(),
            );
        }

        Ok(CSSAstGenerated { code, sourcemap })
    }
}

pub struct CSSAstGenerated {
    pub code: String,
    pub sourcemap: String,
}
