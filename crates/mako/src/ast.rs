use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use base64::engine::general_purpose;
use base64::Engine;
use pathdiff::diff_paths;
use swc_common::errors::Handler;
use swc_common::{FileName, Mark, Span, Spanned, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use swc_css_parser::parser::ParserConfig;
use swc_ecma_ast::Module;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{EsConfig, Parser, StringInput, Syntax, TsConfig};
use swc_error_reporters::{GraphicalReportHandler, PrettyEmitter, PrettyEmitterConfig};
use thiserror::Error;

use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::sourcemap::build_source_map;

#[derive(Debug, Error)]
#[error("{error_message:?}")]
struct ParseError {
    resolved_path: String,
    error_message: String,
}

#[derive(Debug, Clone)]
pub struct Ast {
    pub ast: Module,
    pub unresolved_mark: Mark,
    pub top_level_mark: Mark,
}

pub fn build_js_ast(path: &str, content: &str, context: &Arc<Context>) -> Result<Ast> {
    let absolute_path = PathBuf::from(path);
    let relative_path =
        diff_paths(&absolute_path, &context.config.output.path).unwrap_or(absolute_path);
    let fm = context
        .meta
        .script
        .cm
        .new_source_file(FileName::Real(relative_path), content.to_string());
    let comments = context.meta.script.origin_comments.read().unwrap();
    let is_ts = path.ends_with(".ts") || path.ends_with(".tsx");
    // treat svgr as jsx
    let jsx = path.ends_with(".jsx") || path.ends_with(".svg");
    let tsx = path.ends_with(".tsx");
    let syntax = if is_ts {
        Syntax::Typescript(TsConfig {
            decorators: true,
            tsx,
            ..Default::default()
        })
    } else {
        Syntax::Es(EsConfig {
            jsx,
            ..Default::default()
        })
    };
    let lexer = Lexer::new(
        syntax,
        swc_ecma_ast::EsVersion::Es2015,
        StringInput::from(&*fm),
        Some(comments.get_swc_comments()),
    );
    let mut parser = Parser::new_from(lexer);

    // parse to ast
    let ast = parser.parse_module();
    let mut ast_errors = parser.take_errors();
    if ast.is_err() {
        ast_errors.push(ast.clone().unwrap_err());
    }
    if !ast_errors.is_empty() {
        let mut error_message = vec![];
        for err in ast_errors {
            error_message.push(generate_code_frame(
                err.span(),
                err.kind().msg().to_string().as_str(),
                context.meta.script.cm.clone(),
            ));
        }
        return Err(anyhow!(ParseError {
            resolved_path: path.to_string(),
            error_message: error_message.join("\n"),
        }));
    }

    // top level mark、unresolved mark 需要持久化起来，后续的 transform 需要用到
    GLOBALS.set(&context.meta.script.globals, || {
        let top_level_mark = Mark::new();
        let unresolved_mark = Mark::new();
        Ok(Ast {
            ast: ast.unwrap(),
            unresolved_mark,
            top_level_mark,
        })
    })
}

pub fn build_css_ast(
    path: &str,
    content: &str,
    context: &Arc<Context>,
    css_modules: bool,
) -> Result<Stylesheet> {
    let absolute_path = PathBuf::from(path);
    let relative_path =
        diff_paths(&absolute_path, &context.config.output.path).unwrap_or(absolute_path);
    let fm = context
        .meta
        .css
        .cm
        .new_source_file(FileName::Real(relative_path), content.to_string());
    let config = ParserConfig {
        css_modules,
        legacy_ie: true,
        ..Default::default()
    };
    let lexer = swc_css_parser::lexer::Lexer::new(StringInput::from(&*fm), config);
    let mut parser = swc_css_parser::parser::Parser::new(lexer, config);
    let parse_result = parser.parse_all();

    let mut parse_errors = parser.take_errors();
    if parse_result.is_err() {
        parse_errors.push(parse_result.clone().unwrap_err());
    };
    if !parse_errors.is_empty() {
        let mut error_message = vec![];
        for err in parse_errors {
            error_message.push(generate_code_frame(
                (*err.clone().into_inner()).0,
                err.message().to_string().as_str(),
                context.meta.css.cm.clone(),
            ));
        }
        Err(anyhow!(ParseError {
            resolved_path: path.to_string(),
            error_message: error_message.join("\n"),
        }))
    } else {
        Ok(parse_result.unwrap())
    }
}

pub fn js_ast_to_code(
    ast: &Module,
    context: &Arc<Context>,
    filename: &str,
) -> Result<(String, String)> {
    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();
    {
        let mut emitter = Emitter {
            cfg: JsCodegenConfig::default()
                .with_minify(
                    context.config.minify && matches!(context.config.mode, Mode::Production),
                )
                .with_target(context.config.output.es_version)
                .with_ascii_only(true)
                .with_omit_last_semi(true),
            cm: cm.clone(),
            comments: Some(swc_comments),
            wr: Box::new(JsWriter::new(
                cm.clone(),
                "\n",
                &mut buf,
                Some(&mut source_map_buf),
            )),
        };
        emitter.emit_module(ast)?;
    }

    let sourcemap = match context.config.devtool {
        DevtoolConfig::SourceMap | DevtoolConfig::InlineSourceMap => {
            let src_buf = build_source_map(&source_map_buf, cm);
            String::from_utf8(src_buf).unwrap()
        }
        DevtoolConfig::None => "".to_string(),
    };

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
    let code = String::from_utf8(buf)?;
    Ok((code, sourcemap))
}

pub fn css_ast_to_code(
    ast: &Stylesheet,
    context: &Arc<Context>,
    filename: &str,
) -> (String, String) {
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
            minify: context.config.minify && matches!(context.config.mode, Mode::Production),
        },
    );
    gen.emit(&ast).unwrap();
    let src_buf = build_source_map(&source_map, context.meta.css.cm.clone());
    let sourcemap = String::from_utf8(src_buf).unwrap();

    if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
        // separate sourcemap file
        css_code.push_str(format!("\n/*# sourceMappingURL={filename}.map*/").as_str());
    } else if matches!(context.config.devtool, DevtoolConfig::InlineSourceMap) {
        // inline sourcemap
        css_code.push_str(
            format!(
                "\n/*# sourceMappingURL=data:application/json;charset=utf-8;base64,{}*/",
                base64_encode(&sourcemap)
            )
            .as_str(),
        );
    }
    (css_code, sourcemap)
}

pub fn base64_encode(raw: &str) -> String {
    general_purpose::STANDARD.encode(raw)
}

use swc_common::sync::Lrc;
use swc_common::SourceMap;

pub fn generate_code_frame(span: Span, message: &str, cm: Lrc<SourceMap>) -> String {
    let wr = Box::<LockedWriter>::default();
    let emitter = PrettyEmitter::new(
        cm,
        wr.clone(),
        GraphicalReportHandler::new().with_context_lines(3),
        PrettyEmitterConfig {
            skip_filename: false,
        },
    );
    let handler = Handler::with_emitter(true, false, Box::new(emitter));
    let mut db = handler.struct_span_err(span, message);
    // span.note(format!("Parse file failed: {}", path).as_str());
    db.emit();
    let s = &**wr.0.lock().unwrap();
    s.to_string()
}

#[derive(Clone, Default)]
struct LockedWriter(Arc<Mutex<String>>);

impl fmt::Write for LockedWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.lock().unwrap().push_str(s);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::assert_debug_snapshot;
    use crate::ast::js_ast_to_code;
    use crate::compiler::Context;
    use crate::config::DevtoolConfig;
    use crate::test_helper::create_mock_module;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chinese_ascii() {
        let module = create_mock_module(
            PathBuf::from("/path/to/test"),
            r#"
export const foo = "我是中文";
export const bar = {
    中文: "xxx"
}
"#,
        );
        let mut context = Context::default();
        context.config.devtool = DevtoolConfig::None;
        let (code, _) = js_ast_to_code(
            module.info.unwrap().ast.as_script_mut(),
            &Arc::new(context),
            "testfile.js",
        )
        .unwrap();
        assert_debug_snapshot!(code);
    }
}
