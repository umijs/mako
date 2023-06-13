use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose;
use base64::Engine;
use pathdiff::diff_paths;
use swc_common::FileName;
use swc_css_ast::Stylesheet;
use swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use swc_css_parser::parser::ParserConfig;
use swc_ecma_ast::Module;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsConfig};
use thiserror::Error;

use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::sourcemap::build_source_map;

#[derive(Debug, Error)]
#[error("parse error: {error_message:?} in {resolved_path:?}")]
struct ParseError {
    resolved_path: String,
    error_message: String,
}

pub fn build_js_ast(path: &str, content: &str, context: &Arc<Context>) -> Result<Module> {
    let absolute_path = PathBuf::from(path);
    let relative_path =
        diff_paths(&absolute_path, &context.config.output.path).unwrap_or(absolute_path);
    let fm = context
        .meta
        .script
        .cm
        .new_source_file(FileName::Real(relative_path), content.to_string());
    let syntax = Syntax::Typescript(TsConfig {
        decorators: true,
        tsx: path.ends_with(".tsx") || path.ends_with(".jsx"),
        ..Default::default()
    });
    let lexer = Lexer::new(
        syntax,
        swc_ecma_ast::EsVersion::Es2015,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);

    // parse to ast
    parser.parse_module().map_err(|e| {
        anyhow!(ParseError {
            resolved_path: path.to_string(),
            error_message: format!("{:?}", e),
        })
    })
}

pub fn build_css_ast(path: &str, content: &str, context: &Arc<Context>) -> Result<Stylesheet> {
    let absolute_path = PathBuf::from(path);
    let relative_path =
        diff_paths(&absolute_path, &context.config.output.path).unwrap_or(absolute_path);
    let fm = context
        .meta
        .css
        .cm
        .new_source_file(FileName::Real(relative_path), content.to_string());
    let config = ParserConfig {
        ..Default::default()
    };
    let lexer = swc_css_parser::lexer::Lexer::new(StringInput::from(&*fm), config);
    let mut parser = swc_css_parser::parser::Parser::new(lexer, config);
    parser.parse_all().map_err(|e| {
        anyhow!(ParseError {
            resolved_path: path.to_string(),
            error_message: format!("{:?}", e),
        })
    })
}

pub fn js_ast_to_code(ast: &Module, context: &Arc<Context>, filename: &str) -> (String, String) {
    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    {
        let mut emitter = Emitter {
            cfg: JsCodegenConfig {
                minify: matches!(context.config.mode, Mode::Production),
                ..Default::default()
            },
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(
                cm.clone(),
                "\n",
                &mut buf,
                Some(&mut source_map_buf),
            )),
        };
        emitter.emit_module(ast).unwrap();
    }
    // source map
    let src_buf = build_source_map(&source_map_buf, cm);
    let sourcemap = String::from_utf8(src_buf).unwrap();
    if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
        // separate sourcemap file
        buf.append(
            &mut format!("\n//# sourceMappingURL={filename}.map")
                .as_bytes()
                .to_vec(),
        );
    } else if matches!(context.config.devtool, DevtoolConfig::InlineSourceMap) {
        // inline sourcemap
        buf.append(
            &mut format!(
                "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                base64_encode(&sourcemap)
            )
            .as_bytes()
            .to_vec(),
        );
    }
    let code = String::from_utf8(buf).unwrap();
    (code, sourcemap)
}

pub fn css_ast_to_code(ast: &Stylesheet, context: &Arc<Context>) -> (String, String) {
    let mut css_code = String::new();
    let mut source_map = Vec::new();
    let css_writer = BasicCssWriter::new(
        &mut css_code,
        Some(&mut source_map),
        BasicCssWriterConfig::default(),
    );
    let mut gen = CodeGenerator::new(
        css_writer,
        CodegenConfig {
            minify: matches!(context.config.mode, Mode::Production),
        },
    );
    gen.emit(&ast).unwrap();
    let src_buf = build_source_map(&source_map, context.meta.css.cm.clone());
    let sourcemap = String::from_utf8(src_buf).unwrap();
    (css_code, sourcemap)
}

pub fn base64_encode(raw: &str) -> String {
    general_purpose::STANDARD.encode(raw)
}
