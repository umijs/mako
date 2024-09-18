pub fn decode_xml(s: &str) -> String {
    let mut decoded = String::new();
    html_escape::decode_html_entities_to_string(s, &mut decoded);
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_text() {
        let test_cases = vec![
            ("&amp;", "&"),
            ("&apos;", "'"),
            ("&quot;", "\""),
            ("&gt;", ">"),
            ("&lt;", "<"),
            ("&amp;中文", "&中文"),
            ("&amp;amp;", "&amp;"),
            ("&amp;#38;", "&#38;"),
            ("&amp;#x26;", "&#x26;"),
            ("&#38;#38;", "&#38;"),
            ("&#x26;#38;", "&#38;"),
            ("&#x3a;", ":"),
            ("&>", "&>"),
            ("id=770&#anchor", "id=770&#anchor"),
        ];
        test_cases.into_iter().for_each(|(input, expected)| {
            assert_eq!(decode_xml(input), expected);
        });
    }
}
