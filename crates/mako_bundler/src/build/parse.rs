use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_css_parser::{
    lexer::Lexer as CssLexer,
    parser::{Parser as CssParser, ParserConfig as CssParserConfig},
};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};

use super::load::ContentType;
use crate::context::Context;
use crate::module::ModuleAst;

pub struct ParseParam<'a> {
    pub path: &'a str,
    pub content: String,
    pub content_type: ContentType,
}

pub struct ParseResult {
    pub ast: ModuleAst,
    pub cm: Lrc<SourceMap>,
}

pub fn parse(parse_param: &ParseParam, _context: &Context) -> ParseResult {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom(parse_param.path.to_string()),
        parse_param.content.clone(),
    );
    if matches!(parse_param.content_type, ContentType::Css) {
        let config = CssParserConfig {
            ..Default::default()
        };
        let lexer = CssLexer::new(StringInput::from(&*fm), config);
        let mut parser = CssParser::new(lexer, config);
        let stylesheet = parser
            .parse_all()
            .map_err(|_e| {
                // e.into_diagnostic(&parser.handler).emit();
            })
            .unwrap();
        ParseResult {
            ast: ModuleAst::Css(stylesheet),
            cm,
        }
    } else {
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
        ParseResult {
            ast: ModuleAst::Script(ast),
            cm,
        }
    }
}
