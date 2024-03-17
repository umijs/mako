use mako_core::swc_ecma_ast::*;
use mako_core::swc_ecma_utils::private_ident;
use mako_core::swc_ecma_visit::VisitMut;
use swc_core::common::DUMMY_SP;

pub struct MakoDefaultReactComponent {}
impl MakoDefaultReactComponent {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }
}
impl VisitMut for MakoDefaultReactComponent {
    fn visit_mut_module_item(&mut self, item: &mut ModuleItem) {
        // 将表达式改成函数声明 swc_core 不会认为增加了变量，因此 hygiene_with_config 不会修改重复的变量，因此将代码调整到 module_item
        if let ModuleItem::ModuleDecl(module_decl) = item {
            match module_decl {
                ModuleDecl::ExportDefaultExpr(decl) => {
                    if let Expr::Arrow(ArrowExpr {
                        params,
                        body,
                        is_async,
                        is_generator,
                        return_type,
                        type_params,
                        span,
                        ..
                    }) = *decl.expr.clone()
                    {
                        *item = ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(
                            ExportDefaultDecl {
                                span: DUMMY_SP,
                                decl: DefaultDecl::Fn(FnExpr {
                                    ident: Some(private_ident!("Component$$")), // 无名的默认导出函数
                                    function: Box::new(Function {
                                        params: params
                                            .iter()
                                            .cloned()
                                            .map(|pat: Pat| Param {
                                                span,
                                                decorators: vec![],
                                                pat,
                                            })
                                            .collect::<Vec<_>>(),
                                        body: Some(match *body {
                                            BlockStmtOrExpr::BlockStmt(block_stmt) => block_stmt,
                                            BlockStmtOrExpr::Expr(expr) => {
                                                BlockStmt {
                                                    span, // 使用正确的 span
                                                    stmts: vec![Stmt::Return(ReturnStmt {
                                                        span, // 使用正确的 span
                                                        arg: Some(expr),
                                                    })],
                                                }
                                            }
                                        }),
                                        is_async,
                                        is_generator,
                                        span,
                                        return_type,
                                        type_params,
                                        decorators: vec![],
                                    }),
                                }),
                            },
                        ));
                    }
                }
                ModuleDecl::ExportDefaultDecl(decl) => {
                    if let DefaultDecl::Fn(fn_expr) = &mut decl.decl {
                        if fn_expr.ident.is_none() {
                            fn_expr.ident = Some(private_ident!("Component$$"));
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mako_core::swc_common::sync::Lrc;
    use mako_core::swc_common::SourceMap;
    use mako_core::swc_ecma_ast::*;
    use mako_core::swc_ecma_codegen::text_writer::JsWriter;
    use mako_core::swc_ecma_codegen::{Config, Emitter};
    use mako_core::swc_ecma_transforms::hygiene::hygiene_with_config;
    use mako_core::swc_ecma_visit::VisitMutWith;
    use swc_core::common::comments::NoopComments;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::testing::test;

    use crate::ast_2::tests::TestUtils;
    #[test]
    fn test_normal() {
        run1(r#"export default ()=>{}"#).contains("function");
        run1(r#"export default ()=>{};const Component$$=1;"#).contains("Component$$1");
        run1(r#"const Component$$=1;export default ()=>{};"#).contains("Component$$1");
        assert!(run1(r#"export default function(){}"#).contains("Component$$"));
        assert!(run1(r#"export default function(){} let Component$$ = 1"#).contains("Component$$1"));
        assert!(
            run1(r#"let Component$$ = 1; export default function(){}"#).contains("Component$$1")
        );
    }
    fn run1(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code.to_string());
        let ast = test_utils.ast.js_mut();
        let mut analyzer = super::MakoDefaultReactComponent::new();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            ast.ast.visit_mut_children_with(&mut analyzer);
        });
        // 使用 hygiene 模块修改重复的变量名
        let config = mako_core::swc_ecma_transforms::hygiene::Config {
            ..Default::default()
        };
        let mut hygiene_transform = hygiene_with_config(config);
        ast.ast.visit_mut_with(&mut hygiene_transform);
        module_to_string(&ast.ast)
    }
    pub fn module_to_string(module: &Module) -> String {
        // 初始化 SourceMap 和 Handler，它们对于 Emitter 是必要的

        // 初始化一个缓存字符串，它将存储生成的代码
        let mut output_buf = vec![];

        {
            // 创建一个代码生成器（Emitter）来写入代码到缓存字符串
            let cfg = Config::default();
            let writer = Box::new(JsWriter::new(
                Lrc::new(SourceMap::default()),
                "\n",
                &mut output_buf,
                None,
            ));
            let mut emitter = Emitter {
                cfg, // 你可以配置输出选项
                comments: Some(&NoopComments),
                cm: Lrc::new(SourceMap::default()),
                wr: writer,
            };

            // 使用 Emitter 将 Module 转换为代码
            emitter.emit_module(module).unwrap(); // 注意：这里忽略错误处理
        }

        // 将缓存字符串转换为实际的 Rust 字符串

        let output = String::from_utf8(output_buf).unwrap();
        println!("output:{}", output);
        output
    }
}
