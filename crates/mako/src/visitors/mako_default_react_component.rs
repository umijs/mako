use mako_core::regex::Regex;
use mako_core::swc_atoms::JsWord;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{FileName, SourceMap};
use mako_core::swc_ecma_ast::*;
use mako_core::swc_ecma_visit::VisitMut;
use swc_core::common::Spanned;

pub struct MakoDefaultReactComponent {
    js_word_indents: Vec<JsWord>,
    cm: Lrc<SourceMap>,
}
impl MakoDefaultReactComponent {
    pub fn new(cm: Lrc<SourceMap>) -> Self {
        Self {
            js_word_indents: vec![],
            cm,
        }
    }
}
impl VisitMut for MakoDefaultReactComponent {
    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if !self.js_word_indents.contains(&ident.sym) {
            self.js_word_indents.push(ident.sym.clone());
        }
    }
    fn visit_mut_module_decl(&mut self, decl: &mut ModuleDecl) {
        if let FileName::Real(path) = &self.cm.lookup_char_pos(decl.span().lo).file.name {
            let str = path.to_str().unwrap();
            let extension = path.extension().unwrap();
            let hidden_file_reg = Regex::new(r"(^|/)\.[^/.]").unwrap();
            if (extension != "jsx" && extension != "tsx")
                || str.contains("node_modules")
                || hidden_file_reg.is_match(str)
            {
                return;
            }
        } else {
            return;
        }
        if let ModuleDecl::ExportDefaultDecl(default_expr) = decl {
            if let DefaultDecl::Fn(fn_expr) = &mut default_expr.decl {
                if fn_expr.ident.is_none() {
                    let mut counter = 1;
                    let js_str = "Component$$";
                    let mut default_name = JsWord::from(format!("{}{}", js_str, counter));
                    while self.js_word_indents.contains(&default_name) {
                        default_name = JsWord::from(format!("{}{}", js_str, counter));
                        counter += 1;
                    }
                    let ident = Ident::new(default_name, fn_expr.function.span);
                    fn_expr.ident = Some(ident.clone());
                    self.js_word_indents.push(ident.sym);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mako_core::swc_common::errors::{ColorConfig, Handler};
    use mako_core::swc_common::sync::Lrc;
    use mako_core::swc_common::{FileName, SourceMap};
    use mako_core::swc_ecma_ast::*;
    use mako_core::swc_ecma_codegen::text_writer::JsWriter;
    use mako_core::swc_ecma_codegen::{Config, Emitter};
    use mako_core::swc_ecma_parser::lexer::Lexer;
    use mako_core::swc_ecma_parser::{Parser, StringInput, Syntax};
    use mako_core::swc_ecma_visit::VisitMutWith;
    use swc_core::common::comments::NoopComments;
    use swc_core::ecma::transforms::testing::test;

    #[test]
    fn test_normal() {
        assert_eq!(
            run("export default function(){} let a=1;let b=2").as_str(),
            "export default function Component$$1() {}\nlet a = 1;\nlet b = 2;\n"
        );
    }
    fn run(js_code: &str) -> String {
        let cm: Lrc<SourceMap> = Default::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, false, false, Some(cm.clone()));

        let fm: Lrc<swc_core::common::SourceFile> = cm.new_source_file(
            FileName::Real("path/to/your/file.jsx".into()),
            js_code.into(),
        );
        let lexer = Lexer::new(
            // We want to parse ecmascript
            Syntax::Es(Default::default()),
            // EsVersion defaults to es5
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);

        for e in parser.take_errors() {
            e.into_diagnostic(&handler).emit();
        }
        // GLOBALS.set(&Globals::new(), || {
        let mut module = parser
            .parse_module()
            .map_err(|e| {
                // Unrecoverable fatal error occurred
                e.into_diagnostic(&handler).emit()
            })
            .expect("failed to parser module");
        // 创建 Emitter 实例

        // 获取生成的字符串
        // let mark = module.span.ctxt().outer();
        let mut default = super::MakoDefaultReactComponent::new(cm);
        module.visit_mut_children_with(&mut default);
        module_to_string(&module)
        // })
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
