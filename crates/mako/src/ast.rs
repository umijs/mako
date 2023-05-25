use swc_common::comments::NoopComments;
use swc_common::sync::Lrc;
use swc_common::{FileName, Globals, Mark, SourceMap, DUMMY_SP, GLOBALS};
use swc_css_ast::Stylesheet;
use swc_css_codegen::writer::basic::{BasicCssWriter, BasicCssWriterConfig};
use swc_css_codegen::{CodeGenerator, CodegenConfig, Emit};
use swc_css_parser::parser::ParserConfig;
use swc_ecma_ast::{
    BlockStmt, CallExpr, Expr, ExprOrSpread, ExprStmt, FnExpr, Function, Ident, Lit, Module,
    ModuleItem, Stmt, Str,
};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsConfig};
use swc_ecma_transforms::resolver;
use swc_ecma_transforms::typescript::strip_with_jsx;
use swc_ecma_visit::VisitMutWith;

#[derive(Debug)]
#[allow(dead_code)]
struct ParseError {
    resolved_path: String,
    source: String,
}

pub fn build_js_ast(path: &str, content: &str) -> (Lrc<SourceMap>, Module) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(path.to_string()), content.to_string());
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
    let ast = parser.parse_module().map_err(|e| ParseError {
        resolved_path: path.to_string(),
        source: format!("{:?}", e),
    });
    let ast = ast.unwrap();
    (cm, ast)
}

pub fn build_css_ast(path: &str, content: &str) -> (Lrc<SourceMap>, Stylesheet) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(path.to_string()), content.to_string());
    let config = ParserConfig {
        ..Default::default()
    };
    let lexer = swc_css_parser::lexer::Lexer::new(StringInput::from(&*fm), config);
    let mut parser = swc_css_parser::parser::Parser::new(lexer, config);
    let stylesheet = parser
        .parse_all()
        .map_err(|_e| {
            // e.into_diagnostic(&parser.handler).emit();
        })
        .unwrap();
    (cm, stylesheet)
}

#[allow(dead_code)]
pub fn test_ast() {
    let path = "test.ts";
    let content = include_str!("runtime/runtime_entry.ts");

    // code to parser
    let (cm, mut ast) = build_js_ast(path, content);

    // transform
    let globals = Globals::default();
    GLOBALS.set(&globals, || {
        let top_level_mark = Mark::new();
        let unresolved_mark = Mark::new();
        ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
        ast.visit_mut_with(&mut strip_with_jsx(
            cm.clone(),
            Default::default(),
            NoopComments,
            top_level_mark,
        ));
    });

    // add define wrapper by construct
    // define("test", function() {})
    let body = ast.clone().body;
    let stmts: Vec<Stmt> = body
        .iter()
        .map(|stmt| stmt.as_stmt().unwrap().clone())
        .collect();
    let call_expr = Expr::Call(CallExpr {
        span: DUMMY_SP,
        callee: swc_ecma_ast::Callee::Expr(Box::new(Expr::Ident(Ident {
            span: DUMMY_SP,
            sym: "define".into(),
            optional: false,
        }))),
        args: vec![
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Lit(Lit::Str(Str::from("test")))),
            },
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Fn(FnExpr {
                    ident: None,
                    function: Box::new(Function {
                        params: vec![],
                        decorators: vec![],
                        span: DUMMY_SP,
                        body: Some(BlockStmt {
                            span: DUMMY_SP,
                            stmts,
                        }),
                        is_generator: false,
                        is_async: false,
                        type_params: None,
                        return_type: None,
                    }),
                })),
            },
        ],
        type_args: None,
    });
    let stmt = ModuleItem::Stmt(Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(call_expr),
    }));
    ast.body = vec![stmt];

    // add define wrapper
    // let (_, mut ast2) = build_ast(path, "function define() {}\ndefine(\"test\", function() {});");
    // for stmt in &mut ast2.body {
    //     if let ModuleItem::Stmt(Stmt::Expr(expr)) = stmt {
    //         if let ExprStmt {
    //             expr: box Expr::Call(call_expr),
    //             ..
    //         } = expr {
    //             if let ExprOrSpread {
    //                 expr: box Expr::Fn(func),
    //                 ..
    //             } = &mut call_expr.args[1] {
    //                 let body = ast.clone().body;
    //                 let stmts: Vec<Stmt> = body.iter().map(|stmt| stmt.as_stmt().unwrap().clone()).collect();
    //                 func.function.body.as_mut().unwrap().stmts.extend(stmts);
    //             }
    //         }
    //     }
    // }

    // ast to code
    let (code, sourcemap) = js_ast_to_code(&ast, &cm);
    println!("code: \n\n{}", code);
    println!("source map: \n\n{}", sourcemap);
}

pub fn js_ast_to_code(ast: &Module, cm: &Lrc<SourceMap>) -> (String, String) {
    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    {
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(
                cm.clone(),
                "\n",
                &mut buf,
                Some(&mut source_map_buf),
            )),
        };
        emitter.emit_module(&ast).unwrap();
    }
    let code = String::from_utf8(buf).unwrap();
    let mut src_buf = vec![];
    cm.build_source_map(&mut source_map_buf)
        .to_writer(&mut src_buf)
        .unwrap();
    let sourcemap = String::from_utf8(src_buf).unwrap();
    (code, sourcemap)
}

pub fn css_ast_to_code(ast: &Stylesheet) -> String {
    let mut css_code = String::new();
    let css_writer = BasicCssWriter::new(&mut css_code, None, BasicCssWriterConfig::default());
    let mut gen = CodeGenerator::new(css_writer, CodegenConfig::default());
    gen.emit(&ast).unwrap();
    css_code
}

// #[cfg(test)]
// mod tests {
//     use super::build_js_ast;

//     #[test]
//     fn test_build_js_ast() {
//         let path = "/Users/chencheng/Documents/Code/test/mako-next/node_modules/.pnpm/axios@1.3.6/node_modules/axios/dist/browser/axios.cjs";
//         let content = include_str!("/Users/chencheng/Documents/Code/test/mako-next/node_modules/.pnpm/axios@1.3.6/node_modules/axios/dist/browser/axios.cjs");
//         build_js_ast(path, content);
//     }
// }
