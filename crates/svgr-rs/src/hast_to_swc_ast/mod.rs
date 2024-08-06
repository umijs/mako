use std::collections::HashMap;

use regex::{Captures, Regex};
use swc_core::common::{SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::atoms::JsWord;
use swc_xml::visit::{Visit, VisitWith};

mod decode_xml;
mod mappings;
mod string_to_object_style;
mod util;

use self::decode_xml::*;
use self::mappings::*;
use self::string_to_object_style::*;
use self::util::*;

fn kebab_case(str: &str) -> String {
    let kebab_regex = Regex::new(r"[A-Z\u00C0-\u00D6\u00D8-\u00DE]").unwrap();
    kebab_regex
        .replace_all(str, |caps: &Captures| {
            format!("-{}", &caps[0].to_lowercase())
        })
        .to_string()
}

fn convert_aria_attribute(kebab_key: &str) -> String {
    let parts: Vec<&str> = kebab_key.split('-').collect();
    let aria = parts[0];
    let lowercase_parts: String = parts[1..].join("").to_lowercase();
    format!("{}-{}", aria, lowercase_parts)
}

fn replace_spaces(s: &str) -> String {
    let spaces_regex = Regex::new(r"[\t\r\n\u0085\u2028\u2029]+").unwrap();
    spaces_regex.replace_all(s, |_: &Captures| " ").to_string()
}

fn get_value(attr_name: &str, value: &JsWord) -> JSXAttrValue {
    if attr_name == "style" {
        let style = string_to_object_style(value);

        return JSXAttrValue::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(style)),
        });
    }

    if is_numeric(value) {
        return JSXAttrValue::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(Expr::Lit(Lit::Num(Number {
                span: DUMMY_SP,
                value: value.parse().unwrap(),
                raw: None,
            })))),
        });
    }

    JSXAttrValue::Lit(Lit::Str(Str {
        span: DUMMY_SP,
        value: replace_spaces(value).into(),
        raw: None,
    }))
}

fn text(n: &swc_xml::ast::Text) -> Option<JSXElementChild> {
    let value = n.data.to_string();

    let space_regex = Regex::new(r"^\s+$").unwrap();
    if space_regex.is_match(&value) {
        return None;
    }

    Some(JSXElementChild::JSXExprContainer(JSXExprContainer {
        span: DUMMY_SP,
        expr: JSXExpr::Expr(Box::new(Expr::Lit(Lit::Str(Str {
            span: DUMMY_SP,
            value: decode_xml(&value).into(),
            raw: None,
        })))),
    }))
}

pub struct HastVisitor {
    jsx: Option<JSXElement>,
    attr_mappings: HashMap<&'static str, &'static str>,
}

impl HastVisitor {
    fn new() -> Self {
        Self {
            jsx: None,
            attr_mappings: create_attr_mappings(),
        }
    }

    pub fn get_jsx(&self) -> Option<JSXElement> {
        self.jsx.clone()
    }

    fn element(&self, n: &swc_xml::ast::Element) -> JSXElement {
        let attrs = n
            .attributes
            .iter()
            .map(|attr| {
                let value = attr.value.clone().map(|v| get_value(&attr.name, &v));
                JSXAttrOrSpread::JSXAttr(JSXAttr {
                    span: DUMMY_SP,
                    name: JSXAttrName::Ident(self.get_key(&attr.name, &n.tag_name).into()),
                    value,
                })
            })
            .collect::<Vec<JSXAttrOrSpread>>();

        let name = JSXElementName::Ident(Ident::new(
            n.tag_name.clone(),
            DUMMY_SP,
            SyntaxContext::empty(),
        ));
        let children = self.all(&n.children);

        let opening = JSXOpeningElement {
            span: DUMMY_SP,
            name: name.clone(),
            attrs,
            self_closing: children.is_empty(),
            type_args: None,
        };

        let closing = if !children.is_empty() {
            Some(JSXClosingElement {
                span: DUMMY_SP,
                name,
            })
        } else {
            None
        };

        JSXElement {
            span: DUMMY_SP,
            opening,
            children,
            closing,
        }
    }

    fn all(&self, children: &[swc_xml::ast::Child]) -> Vec<JSXElementChild> {
        children
            .iter()
            .filter_map(|n| match n {
                swc_xml::ast::Child::Element(e) => {
                    Some(JSXElementChild::JSXElement(Box::new(self.element(e))))
                }
                swc_xml::ast::Child::Text(t) => text(t),
                _ => None,
            })
            .collect()
    }

    fn get_key(&self, attr_name: &str, tag_name: &str) -> Ident {
        let lower_case_name = attr_name.to_lowercase();
        let rc_key = {
            match tag_name {
                "input" => match lower_case_name.as_str() {
                    "checked" => Some("defaultChecked"),
                    "value" => Some("defaultValue"),
                    "maxlength" => Some("maxLength"),
                    _ => None,
                },
                "form" => match lower_case_name.as_str() {
                    "enctype" => Some("encType"),
                    _ => None,
                },
                _ => None,
            }
        };

        if let Some(k) = rc_key {
            return Ident {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                sym: k.into(),
                optional: false,
            };
        }

        let mapped_attr = self.attr_mappings.get(lower_case_name.as_str());
        if let Some(k) = mapped_attr {
            return Ident {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                sym: JsWord::from(*k),
                optional: false,
            };
        }

        let kebab_key = kebab_case(attr_name);

        if kebab_key.starts_with("aria-") {
            return Ident {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                sym: convert_aria_attribute(attr_name).into(),
                optional: false,
            };
        }

        if kebab_key.starts_with("data-") {
            return Ident {
                span: DUMMY_SP,
                ctxt: SyntaxContext::empty(),
                sym: attr_name.into(),
                optional: false,
            };
        }

        Ident {
            span: DUMMY_SP,
            ctxt: SyntaxContext::empty(),
            sym: attr_name.into(),
            optional: false,
        }
    }
}

impl Visit for HastVisitor {
    fn visit_element(&mut self, n: &swc_xml::ast::Element) {
        self.jsx = Some(self.element(n));
    }
}

pub fn to_swc_ast(hast: swc_xml::ast::Document) -> Option<JSXElement> {
    let mut v = HastVisitor::new();
    hast.visit_with(&mut v);
    v.get_jsx()
}

#[cfg(test)]
mod tests {
    use std::borrow::Borrow;
    use std::path::PathBuf;
    use std::sync::Arc;

    use swc_core::common::{FileName, SourceFile, SourceMap};
    use swc_core::ecma::codegen::text_writer::JsWriter;
    use swc_core::ecma::codegen::{Config, Emitter};
    use swc_xml::parser::parse_file_as_document;
    use testing::NormalizedOutput;

    use super::*;

    fn transform(cm: Arc<SourceMap>, fm: Arc<SourceFile>, minify: bool) -> String {
        let mut errors = vec![];
        let doc = parse_file_as_document(fm.borrow(), Default::default(), &mut errors).unwrap();

        let jsx = to_swc_ast(doc).unwrap();

        let mut buf = vec![];

        let new_line = match minify {
            true => "",
            false => "\n",
        };
        let mut emitter = Emitter {
            cfg: Config::default().with_minify(minify),
            cm: cm.clone(),
            comments: None,
            wr: JsWriter::new(cm, new_line, &mut buf, None),
        };

        emitter
            .emit_module_item(&ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(Expr::JSXElement(Box::new(jsx))),
            })))
            .unwrap();

        String::from_utf8_lossy(&buf).to_string()
    }

    fn document_test(input: PathBuf) {
        let jsx_path = input.parent().unwrap().join("output.jsx");

        let cm = Arc::<SourceMap>::default();
        let fm = cm.load_file(&input).expect("failed to load fixture file");

        let res = transform(cm, fm, false);

        NormalizedOutput::from(res)
            .compare_to_file(jsx_path)
            .unwrap();
    }

    fn code_test(input: &str, expected: &str) {
        let cm = Arc::<SourceMap>::default();
        let fm = cm.new_source_file(FileName::Anon.into(), input.to_string());

        let res = transform(cm, fm, true);

        assert_eq!(res, expected)
    }

    #[testing::fixture("__fixture__/*/*.svg")]
    fn pass(input: PathBuf) {
        document_test(input);
    }

    #[test]
    fn transforms_data_x() {
        code_test(
            r#"<svg data-hidden="true"></svg>"#,
            r#"<svg data-hidden="true"/>;"#,
        );
    }

    #[test]
    fn preserves_mask_type() {
        code_test(
            r#"<svg><mask mask-type="alpha"/></svg>"#,
            r#"<svg><mask mask-type="alpha"/></svg>;"#,
        );
    }

    #[test]
    fn string_literals_children_of_text_nodes_should_have_decoded_xml_entities() {
        code_test(
            r#"<svg><text>&lt;</text></svg>"#,
            r#"<svg><text>{"<"}</text></svg>;"#,
        );
    }

    #[test]
    fn string_literals_children_of_tspan_nodes_should_have_decoded_xml_entities() {
        code_test(
            r#"<svg><text><tspan>&lt;</tspan></text></svg>"#,
            r#"<svg><text><tspan>{"<"}</tspan></text></svg>;"#,
        );
    }

    #[test]
    fn transforms_style() {
        code_test(
            r#"<svg><path style="--index: 1; font-size: 24px;"></path><path style="--index: 2"></path></svg>"#,
            r#"<svg><path style={{"--index":1,fontSize:24}}/><path style={{"--index":2}}/></svg>;"#,
        );
    }

    #[test]
    fn transforms_class() {
        code_test(
            r#"<svg><path class="icon"/></svg>"#,
            r#"<svg><path className="icon"/></svg>;"#,
        );
    }
}
