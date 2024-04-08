use mako_core::swc_css_ast::{
    ComponentValue, Declaration, DeclarationName, Dimension, Function, FunctionName, Ident,
    Integer, Length, LengthPercentage, Number, Percentage, SimpleBlock,
};
use mako_core::swc_css_visit::{VisitMut, VisitMutWith};

/**
 * Rust version of postcss-flexbugs-fixes
 */
pub struct CSSFlexbugs;

impl CSSFlexbugs {
    fn get_0_percent_value(&self) -> ComponentValue {
        ComponentValue::LengthPercentage(Box::new(LengthPercentage::Percentage(Percentage {
            value: Number {
                value: 0.0,
                raw: None,
                span: Default::default(),
            },
            span: Default::default(),
        })))
    }

    fn proper_basis(&self, basis: &mut ComponentValue) {
        // transform 0/0px to 0%
        if match basis {
            ComponentValue::Integer(box Integer { value, .. }) => value == &0,
            ComponentValue::Dimension(box Dimension::Length(Length {
                value: Number { value, .. },
                unit: Ident { value: unit, .. },
                ..
            })) => unit == "px" && value == &0.0,
            _ => false,
        } {
            *basis = self.get_0_percent_value();
        }
    }
}

impl VisitMut for CSSFlexbugs {
    fn visit_mut_declaration(&mut self, n: &mut Declaration) {
        if let Declaration {
            name: DeclarationName::Ident(Ident { value: name, .. }),
            value: decl_value,
            ..
        } = n
        {
            if name == "flex"
                // skip reserved words and custom properties
                && !matches!(
                    decl_value.first(),
                    Some(ComponentValue::Ident(..) | ComponentValue::Function(..))
                )
            {
                // same behavior with postcss-flexbugs-fixes bug4 and bug6
                // ref: https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/bugs/bug4.js
                // ref: https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/bugs/bug6.js
                let default_flex_shrink = ComponentValue::Integer(Box::new(Integer {
                    value: 1,
                    raw: None,
                    span: Default::default(),
                }));
                let mut default_flex_basis = self.get_0_percent_value();

                *decl_value = vec![
                    // fallback flex-grow to 0
                    decl_value
                        .first()
                        .unwrap_or(&ComponentValue::Integer(Box::new(Integer {
                            value: 0,
                            raw: None,
                            span: Default::default(),
                        })))
                        .clone(),
                    // fallback flex-shrink to 1
                    decl_value
                        .get(1)
                        .map(|v| match v {
                            ComponentValue::Integer(_) => v.clone(),
                            _ => {
                                // treat non-numeric 2th value as flex-basis
                                default_flex_basis = v.clone();

                                // ignore non-numeric flex-shrink and fallback to 1
                                default_flex_shrink.clone()
                            }
                        })
                        .unwrap_or(default_flex_shrink.clone()),
                    // fallback flex-basis to numeric 2th value or 0%
                    decl_value.get(2).unwrap_or(&default_flex_basis).clone(),
                ];

                // normalize flex-basis
                self.proper_basis(&mut decl_value[2]);

                // Safari seems to hate '0%' and the others seems to make do with a nice value when basis is missing,
                // so if we see a '0%', just remove it.  This way it'll get adjusted for any other cases where '0%' is
                // already defined somewhere else.
                if let Some(ComponentValue::LengthPercentage(box LengthPercentage::Percentage(
                    Percentage {
                        value: Number { value, .. },
                        ..
                    },
                ))) = decl_value.get(2)
                {
                    if value == &0.0 {
                        decl_value.remove(2);
                    }
                }
            }
        }
    }

    fn visit_mut_simple_block(&mut self, n: &mut SimpleBlock) {
        let mut i = 0;

        while i < n.value.len() {
            if let ComponentValue::Declaration(box Declaration {
                name: DeclarationName::Ident(Ident { value: name, .. }),
                value,
                important,
                span,
            }) = &n.value[i]
            {
                if let Some(ComponentValue::Function(box Function {
                    name:
                        FunctionName::Ident(Ident {
                            value: basis_fn_name,
                            ..
                        }),
                    ..
                })) = value.get(2)
                {
                    if name == "flex" && basis_fn_name == "calc" {
                        // same behavior with postcss-flexbugs-fixes bug81a
                        // ref: https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/bugs/bug81a.js
                        n.value.splice(
                            i..i + 1,
                            vec![(0, "flex-grow"), (1, "flex-shrink"), (2, "flex-basis")]
                                .into_iter()
                                .map(|(i, name)| {
                                    ComponentValue::Declaration(Box::new(Declaration {
                                        name: DeclarationName::Ident(Ident {
                                            value: name.into(),
                                            raw: None,
                                            span: Default::default(),
                                        }),
                                        value: vec![value[i].clone()],
                                        important: important.clone(),
                                        span: *span,
                                    }))
                                })
                                .collect::<Vec<_>>(),
                        );

                        // skip expanded declarations
                        i += 2;
                    }
                }
            }

            i += 1;
        }

        n.visit_mut_children_with(self);
    }
}

#[cfg(test)]
mod test {

    use mako_core::swc_css_visit::VisitMutWith;

    use crate::ast_2::tests::TestUtils;

    // migrate from https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/specs/bug4Spec.js
    #[test]
    fn bug4() {
        assert_eq!(
            run("div{flex:1}"),
            "div{flex:1 1}",
            "set 0% for default flex-basis and 1 for flex-shrink in flex shorthand"
        );
        assert_eq!(
            run("div{flex:1}"),
            "div{flex:1 1}",
            "set 0% for default flex-basis and 1 for flex-shrink in flex shorthand"
        );
        assert_eq!(
            run("div{flex:1 1}"),
            "div{flex:1 1}",
            "set 0% for default flex-basis when not specified"
        );
        assert_eq!(
            run("div{flex:1 0 0}"),
            "div{flex:1 0}",
            "set flex-basis === 0% for flex-basis with plain 0"
        );
        assert_eq!(
            run("div{flex:1 0 0px}"),
            "div{flex:1 0}",
            "set flex-basis === 0% for flex-basis with 0px"
        );
        assert_eq!(
            run("div{flex:1 50%}"),
            "div{flex:1 1 50%}",
            "set flex-basis when second value is not a number"
        );

        // do nothing
        assert_eq!(
            run("a{display:flex}"),
            "a{display:flex}",
            "when not flex declarations"
        );
        assert_eq!(
            run("div{flex:1 0 100%}"),
            "div{flex:1 0 100%}",
            "when flex-basis with percent is set"
        );
        assert_eq!(
            run("div{flex:1 0 10px}"),
            "div{flex:1 0 10px}",
            "when flex-basis with pixels is set"
        );
        assert_eq!(
            run("div{flex:1 1 auto}"),
            "div{flex:1 1 auto}",
            "does not change auto flex-basis is set explicitly"
        );

        // when flex value is reserved word
        let string_values = ["none", "auto", "content", "inherit", "initial", "unset"];
        string_values.into_iter().for_each(|s| {
            assert_eq!(
                run(&format!("div{{flex:{}}}", s)),
                format!("div{{flex:{}}}", s),
                "does not change for flex:{}",
                s
            );
        });

        assert_eq!(
            run("div{flex:var(--foo)}"),
            "div{flex:var(--foo)}",
            "is a custom property"
        );
    }

    // migrate from https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/specs/bug6Spec.js
    #[test]
    fn bug6() {
        assert_eq!(
            run("div{flex:1}"),
            "div{flex:1 1}",
            "Set flex-shrink to 1 by default"
        );

        // do nothing
        assert_eq!(
            run("div{flex:none}"),
            "div{flex:none}",
            "when flex is set to none"
        );

        assert_eq!(
            run("div{flex:1 0 0%}"),
            "div{flex:1 0 0%}",
            "when flex-shrink is set explicitly to zero"
        );

        assert_eq!(
            run("div{flex:1 2 0%}"),
            "div{flex:1 2 0%}",
            "when flex-shrink is set explicitly to non-zero value"
        );
    }

    // migrate from https://github.com/luisrudge/postcss-flexbugs-fixes/blob/683560e1f0a4e67009331b564d530ccfefb831ad/specs/bug81aSpec.js
    #[test]
    fn bug81a() {
        assert_eq!(
            run("a{flex:1 0 calc(1vw - 1px)}"),
            "a{flex-grow:1;flex-shrink:0;flex-basis:calc(1vw - 1px)}",
            "Expands the shorthand when calc() is used"
        );

        // do nothing
        assert_eq!(
            run("a{flex:0}"),
            "a{flex:0 1}",
            "when using only first value"
        );

        assert_eq!(
            run("a{flex:0 0}"),
            "a{flex:0 0}",
            "when using only first and second values"
        );

        assert_eq!(
            run("a{flex:0 0 1px}"),
            "a{flex:0 0 1px}",
            "when not using calc"
        );
    }

    fn run(css_code: &str) -> String {
        let mut test_utils = TestUtils::gen_css_ast(css_code.to_string(), true);
        let ast = test_utils.ast.css_mut();
        let mut visitor = super::CSSFlexbugs {};
        ast.ast.visit_mut_with(&mut visitor);
        test_utils.css_ast_to_code()
    }
}
