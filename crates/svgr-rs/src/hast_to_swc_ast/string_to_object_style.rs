use regex::{Captures, Regex};
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::*;

use super::util::*;

const PX_REGEX: &str = r#"^\d+px$"#;
const MS_REGEX: &str = r#"^-ms-"#;
const VAR_REGEX: &str = r#"^--"#;

pub fn hyphen_to_camel_case(s: &str) -> String {
    let regex = Regex::new(r#"-(.)"#).unwrap();
    regex
        .replace_all(s, |caps: &Captures| caps[1].to_uppercase())
        .into()
}

// Format style key into JSX style object key.
pub fn format_key(key: &str) -> PropName {
    let var_regex = Regex::new(VAR_REGEX).unwrap();
    if var_regex.is_match(key) {
        return PropName::Str(Str {
            span: DUMMY_SP,
            value: key.into(),
            raw: None,
        });
    }

    let mut key = key.to_lowercase();
    let ms_regex = Regex::new(MS_REGEX).unwrap();
    if ms_regex.is_match(&key) {
        key = key[1..].into();
    }

    PropName::Ident(IdentName::new(hyphen_to_camel_case(&key).into(), DUMMY_SP))
}

fn is_convertible_pixel_value(s: &str) -> bool {
    let px_regex = Regex::new(PX_REGEX).unwrap();
    px_regex.is_match(s)
}

// Format style value into JSX style object value.
pub fn format_value(value: &str) -> Expr {
    if is_numeric(value) {
        return Expr::Lit(Lit::Num(Number {
            span: DUMMY_SP,
            value: value.parse().unwrap(),
            raw: None,
        }));
    }

    if is_convertible_pixel_value(value) {
        return Expr::Lit(Lit::Num(Number {
            span: DUMMY_SP,
            value: value[..value.len() - 2].parse().unwrap(),
            raw: None,
        }));
    }

    Expr::Lit(Lit::Str(Str {
        span: DUMMY_SP,
        value: value.into(),
        raw: None,
    }))
}

pub fn string_to_object_style(raw_style: &str) -> Expr {
    let entries = raw_style.split(';');

    let properties = entries
        .into_iter()
        .filter_map(|entry| {
            let style = entry.trim();
            if style.is_empty() {
                return None;
            }

            let first_colon = style.find(':');
            match first_colon {
                Some(i) => {
                    let value = format_value(style[(i + 1)..].trim());
                    let key = format_key(style[..i].trim());

                    Some(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                        key,
                        value: Box::new(value),
                    }))))
                }
                None => None,
            }
        })
        .collect::<Vec<PropOrSpread>>();

    Expr::Object(ObjectLit {
        span: DUMMY_SP,
        props: properties,
    })
}
