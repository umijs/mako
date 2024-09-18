pub fn decode_xml(s: &str) -> String {
    let chars = s.chars().collect::<Vec<_>>();

    let mut ret = String::new();
    let mut cur_idx = 0;
    let mut last_idx = 0;

    while cur_idx < chars.len() {
        if chars[cur_idx] != '&' {
            cur_idx += 1;
            continue;
        }

        ret.push_str(&s[last_idx..cur_idx]);
        last_idx = cur_idx;
        // Skip the "&"
        cur_idx += 1;

        // If we have a numeric entity, handle this separately.
        if chars[cur_idx] == '#' {
            // Skip the leading "&#". For hex entities, also skip the leading "x".
            cur_idx += 1;

            let mut start = cur_idx;
            let mut radix = 10;

            if chars[start].to_ascii_lowercase() == 'x' {
                radix = 16;
                cur_idx += 1;
                start += 1;
            }

            while (chars[cur_idx] >= '0' && chars[cur_idx] <= '9')
                || (radix == 16
                    && chars[cur_idx].to_ascii_lowercase() >= 'a'
                    && chars[cur_idx].to_ascii_lowercase() <= 'f')
            {
                cur_idx += 1;
            }

            if start != cur_idx {
                let entity = &s[start..cur_idx];
                if chars[cur_idx] != ';' {
                    continue;
                }

                cur_idx += 1;
                last_idx = cur_idx;

                let parsed = u32::from_str_radix(entity, radix).unwrap();
                ret.push(char::from_u32(parsed).unwrap());
            }

            continue;
        }

        // &gt;
        // &lt;
        if chars.len() - cur_idx > 4 {
            match &s[cur_idx..cur_idx + 4] {
                "gt;" => {
                    ret.push('>');
                    cur_idx += 3;
                    last_idx = cur_idx;
                }
                "lt;" => {
                    ret.push('<');
                    cur_idx += 3;
                    last_idx = cur_idx;
                }
                _ => {}
            }
        }

        // &amp;
        if chars.len() - cur_idx > 5 && s[cur_idx..cur_idx + 4].to_string() == "amp;" {
            ret.push('&');
            cur_idx += 4;
            last_idx = cur_idx;
        }

        // &apos;
        // &quot;
        if chars.len() - cur_idx > 6 {
            match s[cur_idx..cur_idx + 4].into() {
                "apos;" => {
                    ret.push('\'');
                    cur_idx += 5;
                    last_idx = cur_idx;
                }
                "quot;" => {
                    ret.push('\\');
                    cur_idx += 5;
                    last_idx = cur_idx;
                }
                _ => {}
            }
        }
    }

    ret + &s[last_idx..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_text() {
        let test_cases = vec![
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
