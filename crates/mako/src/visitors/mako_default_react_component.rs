use mako_core::swc_atoms::JsWord;
use mako_core::swc_ecma_ast::*;
use mako_core::swc_ecma_visit::VisitMut;

pub struct MakoDefaultReactComponent {}
impl MakoDefaultReactComponent {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }
}
impl VisitMut for MakoDefaultReactComponent {
    fn visit_mut_export_default_decl(&mut self, decl: &mut ExportDefaultDecl) {
        if let DefaultDecl::Fn(fn_expr) = &mut decl.decl {
            if fn_expr.ident.is_none() {
                let ident = Ident::new(JsWord::from("Component$$"), fn_expr.function.span);
                fn_expr.ident = Some(ident.clone());
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
        assert!(run1(r#"export default function(){}"#).contains("Component$$"));
        assert!(run1(r#"export default function(){} let Component$$ = 1"#).contains("Component$$1"));
        assert!(run1(r#"let Component$$ = 1; export default function(){}"#).contains("Component$$1"));
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

        output
    }
}
