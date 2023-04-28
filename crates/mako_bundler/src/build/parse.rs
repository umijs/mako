use std::sync::Arc;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_ecma_ast::Module;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};

use crate::context::Context;

pub struct ParseParam<'a> {
    pub path: &'a str,
    pub content: String,
}

pub struct ParseResult {
    pub ast: Module,
    pub cm: Lrc<SourceMap>,
}

pub fn parse(parse_param: &ParseParam, _context: &Arc<Context>) -> ParseResult {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom(parse_param.path.to_string()),
        parse_param.content.clone(),
    );
    let lexer = Lexer::new(
        Syntax::Typescript(TsConfig {
            decorators: true,
            tsx: parse_param.path.ends_with(".tsx"),
            ..Default::default()
        }),
        swc_ecma_ast::EsVersion::Es2015,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let ast = parser
        .parse_module()
        .map_err(|_e| {
            // e.into_diagnostic(&parser.handler).emit();
        })
        .unwrap();
    ParseResult { ast, cm }
}
