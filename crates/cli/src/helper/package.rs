pub fn parse_package_name(path: &str) -> (Option<String>, String, String) {
    let parts: Vec<&str> = path.split('/').collect();
    let len = parts.len();

    if len >= 2 {
        let last = parts[len - 1];
        let second_last = parts[len - 2];

        if second_last.starts_with('@') {
            // scoped package: @scope/name
            (
                Some(second_last.to_string()),
                last.to_string(),
                format!("{}/{}", second_last, last),
            )
        } else {
            // normal package
            (None, last.to_string(), last.to_string())
        }
    } else if len == 1 {
        // name only
        (None, parts[0].to_string(), parts[0].to_string())
    } else {
        // invalid path
        (None, path.to_string(), path.to_string())
    }
}
