use std::collections::HashMap;
use std::panic;

use swc_common::collections::AHashMap;
use swc_common::sync::Lrc;
use swc_common::DUMMY_SP;
use swc_common::{Globals, SourceMap, GLOBALS};
use swc_css_ast;
use swc_css_codegen::{
    writer::basic::{BasicCssWriter, BasicCssWriterConfig},
    CodeGenerator, CodegenConfig, Emit,
};
use swc_css_visit::VisitMutWith as CssVisitMutWith;
use swc_ecma_ast::*;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_transforms::helpers::{Helpers, HELPERS};
use swc_ecma_utils::ExprFactory;
use swc_ecma_visit::VisitMutWith;

use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};

use crate::module::{ModuleAst, ModuleId};

use super::dep_replacer::DepReplacer;
use super::env_replacer::EnvReplacer;

pub struct TransformParam<'a> {
    pub id: &'a ModuleId,
    pub ast: &'a ModuleAst,
    pub cm: &'a Lrc<SourceMap>,
    pub dep_map: HashMap<String, String>,
    pub env_map: HashMap<String, String>,
}

pub struct TransformResult {
    pub ast: ModuleAst,
    pub code: String,
}

pub fn transform(transform_param: &TransformParam) -> TransformResult {
    match transform_param.ast {
        ModuleAst::Script(ast) => tranform_js(transform_param, ast),
        ModuleAst::Css(ast) => transform_css(transform_param, ast),
        _ => panic!("not supported module"),
    }
}

fn tranform_js(transform_param: &TransformParam, ast: &Module) -> TransformResult {
    let id = transform_param.id;
    let cm = transform_param.cm.clone();
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

    GLOBALS.set(&globals, || {
        let helpers = Helpers::new(true);
        HELPERS.set(&helpers, || {
            let mut dep_replacer = DepReplacer {
                dep_map: transform_param.dep_map.clone(),
            };
            ast.visit_mut_with(&mut dep_replacer);

            let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
            ast.visit_mut_with(&mut env_replacer);

            wrap_module(id, &mut ast);
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
}

fn transform_css(
    transform_param: &TransformParam,
    ast: &swc_css_ast::Stylesheet,
) -> TransformResult {
    let id = transform_param.id;
    let cm = transform_param.cm.clone();
    let mut stylesheet = ast.clone();

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

    //lightingcss
    let mut lightingcss_stylesheet =
        StyleSheet::parse(&css_code, ParserOptions::default()).unwrap();
    lightingcss_stylesheet
        .minify(MinifyOptions::default())
        .unwrap();
    let out = lightingcss_stylesheet
        .to_css(PrinterOptions::default())
        .unwrap();

    let ast = wrap_css(id, &transform_param.dep_map, out.code.as_str());
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
        ast: ModuleAst::Css(stylesheet),
        code,
    }
}

fn wrap_css(id: &ModuleId, dep_map: &HashMap<String, String>, css: &str) -> Module {
    let stylesheet_literal = Str {
        raw: None,
        span: DUMMY_SP,
        value: css.into(),
    };

    let css_code_var = Ident::new("cssCode".into(), DUMMY_SP);

    let css_code_decl = VarDecl {
        span: DUMMY_SP,
        kind: VarDeclKind::Const,
        declare: false,
        decls: vec![VarDeclarator {
            span: DUMMY_SP,
            definite: false,
            name: Pat::Ident(css_code_var.clone().into()),
            init: Some(Box::new(Expr::Lit(Lit::Str(stylesheet_literal)))),
        }],
    };

    let style_var = Ident::new("style".into(), DUMMY_SP);

    let style_var_decl = VarDecl {
        span: DUMMY_SP,
        kind: VarDeclKind::Const,
        decls: vec![VarDeclarator {
            span: DUMMY_SP,
            name: Pat::Ident(style_var.clone().into()),
            init: Some(Box::new(Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Ident::new("document".into(), DUMMY_SP)
                    .make_member(Ident::new("createElement".into(), DUMMY_SP))
                    .as_callee(),
                args: vec![ExprOrSpread {
                    spread: None,
                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                        span: DUMMY_SP,
                        value: "style".into(),
                        raw: None,
                    }))),
                }],
                type_args: None,
            }))),
            definite: false,
        }],
        declare: false,
    };

    let style_inner_html_assign = Expr::Assign(AssignExpr {
        span: DUMMY_SP,
        op: op!("="),
        left: PatOrExpr::Expr(Box::new(Expr::Member(MemberExpr {
            span: DUMMY_SP,
            obj: Box::new(Expr::Ident(style_var.clone().into())),
            prop: MemberProp::Ident(Ident::new("innerHTML".into(), DUMMY_SP)),
        }))),
        right: Box::new(Expr::Ident(css_code_var.into())),
    });

    let append_child_call = Expr::Call(CallExpr {
        span: DUMMY_SP,
        callee: Ident::new("document".into(), DUMMY_SP)
            .make_member(Ident::new("head".into(), DUMMY_SP))
            .make_member(Ident::new("appendChild".into(), DUMMY_SP))
            .as_callee(),
        args: vec![style_var.as_arg()],
        type_args: Default::default(),
    });

    let mut body: Vec<ModuleItem> = dep_map
        .values()
        .map(|value| {
            ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(Expr::Call(CallExpr {
                    span: DUMMY_SP,
                    callee: Ident::new("require".into(), DUMMY_SP).as_callee(),
                    args: vec![Lit::Str(Str {
                        span: DUMMY_SP,
                        value: value.as_str().into(),
                        raw: None,
                    })
                    .as_arg()],
                    type_args: None,
                })),
            }))
        })
        .collect();

    body.extend(vec![
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(css_code_decl)))),
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(style_var_decl)))),
        ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(style_inner_html_assign),
        })),
        ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(append_child_call),
        })),
    ]);

    let mut ast = Module {
        span: DUMMY_SP,
        body,
        shebang: None,
    };

    wrap_module(id, &mut ast);
    ast
}

fn wrap_module(id: &ModuleId, dep: &mut Module) {
    let id = id.id.clone();

    let module_fn = Expr::Fn(FnExpr {
        ident: None,
        function: Box::new(Function {
            is_generator: false,
            is_async: false,
            type_params: None,
            return_type: None,
            span: DUMMY_SP,
            decorators: vec![],
            params: vec![
                // module
                Param {
                    span: DUMMY_SP,
                    decorators: Default::default(),
                    pat: Pat::Ident(Ident::new("module".into(), DUMMY_SP).into()),
                },
                // exports
                Param {
                    span: DUMMY_SP,
                    decorators: Default::default(),
                    pat: Pat::Ident(Ident::new("exports".into(), DUMMY_SP).into()),
                },
                // require
                Param {
                    span: DUMMY_SP,
                    decorators: Default::default(),
                    pat: Pat::Ident(Ident::new("require".into(), DUMMY_SP).into()),
                },
            ],
            body: Some(BlockStmt {
                span: dep.span,
                stmts: dep
                    .clone()
                    .body
                    .into_iter()
                    .map(|v| match v {
                        ModuleItem::ModuleDecl(i) => {
                            unreachable!("module item found in none-es6 file: {:?}", i)
                        }
                        ModuleItem::Stmt(s) => s,
                    })
                    .collect(),
            }),
        }),
    });

    let stmt = Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Ident::new("g_define".into(), DUMMY_SP).as_callee(),
            args: vec![
                Lit::Str(Str {
                    span: DUMMY_SP,
                    value: id.into(),
                    raw: None,
                })
                .as_arg(),
                module_fn.as_arg(),
            ],
            type_args: None,
        })),
    });

    *dep = Module {
        span: DUMMY_SP,
        body: vec![ModuleItem::Stmt(stmt)],
        shebang: None,
    };
}
