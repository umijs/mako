use regex::Regex;

pub fn is_numeric(s: &str) -> bool {
    let regex = Regex::new(r#"^(\-|\+)?\d+(\.\d+)?$"#).unwrap();
    regex.is_match(s)
}
