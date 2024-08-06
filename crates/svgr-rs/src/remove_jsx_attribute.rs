use swc_core::ecma::ast::*;
use swc_core::ecma::visit::VisitMut;

use super::core;

pub struct Visitor {
    elements: Vec<String>,
    attributes: Vec<String>,
}

impl Visitor {
    pub fn new(config: &core::config::Config) -> Self {
        let mut attributes = vec!["version".to_string()];

        let dimensions = config.dimensions.unwrap_or(true);
        if !dimensions {
            attributes.push("width".to_string());
            attributes.push("height".to_string());
        }

        Self {
            elements: vec!["svg".to_string(), "Svg".to_string()],
            attributes,
        }
    }
}

impl VisitMut for Visitor {
    fn visit_mut_jsx_opening_element(&mut self, n: &mut JSXOpeningElement) {
        if let JSXElementName::Ident(ident) = &n.name {
            if !self.elements.contains(&ident.sym.to_string()) {
                return;
            }
        } else {
            return;
        }

        let len = n.attrs.len();
        let mut attrs = n.attrs.clone();
        attrs.reverse();
        attrs.iter().enumerate().for_each(|(index, attr)| {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(ident) = &jsx_attr.name {
                    if self.attributes.contains(&ident.sym.to_string()) {
                        n.attrs.remove(len - index - 1);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::default::Default;
    use std::sync::Arc;

    use swc_core::common::{FileName, SourceMap};
    use swc_core::ecma::ast::*;
    use swc_core::ecma::codegen::text_writer::JsWriter;
    use swc_core::ecma::codegen::Emitter;
    use swc_core::ecma::parser::lexer::Lexer;
    use swc_core::ecma::parser::{EsSyntax, Parser, StringInput, Syntax};
    use swc_core::ecma::visit::{as_folder, FoldWith};

    use super::*;

    pub struct Options {
        elements: Vec<String>,
        attributes: Vec<String>,
    }

    fn code_test(input: &str, opts: Options, expected: &str) {
        let cm = Arc::new(SourceMap::default());
        let fm = cm.new_source_file(FileName::Anon.into(), input.to_string());

        let lexer = Lexer::new(
            Syntax::Es(EsSyntax {
                decorators: true,
                jsx: true,
                ..Default::default()
            }),
            EsVersion::EsNext,
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().unwrap();

        let module = module.fold_with(&mut as_folder(Visitor {
            elements: opts.elements,
            attributes: opts.attributes,
        }));

        let mut buf = vec![];
        let mut emitter = Emitter {
            cfg: Default::default(),
            cm: cm.clone(),
            comments: None,
            wr: JsWriter::new(cm, "", &mut buf, None),
        };
        emitter.emit_module(&module).unwrap();
        let result = String::from_utf8_lossy(&buf).to_string();

        assert_eq!(result, expected);
    }

    #[test]
    fn should_remove_attributes_from_an_element() {
        code_test(
            r#"<div foo><span foo/></div>;"#,
            Options {
                elements: vec!["span".to_string()],
                attributes: vec!["foo".to_string()],
            },
            r#"<div foo><span/></div>;"#,
        );
    }

    #[test]
    fn should_not_throw_error_when_spread_operator_is_used() {
        code_test(
            r#"<div foo><span foo {...props}/></div>;"#,
            Options {
                elements: vec!["span".to_string()],
                attributes: vec!["foo".to_string()],
            },
            r#"<div foo><span {...props}/></div>;"#,
        );
    }
}
