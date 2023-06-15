use crate::config::Config;

fn get_node_builtins() -> Vec<String> {
    vec![
        "assert",
        "buffer",
        "console",
        "constants",
        "crypto",
        "domain",
        "events",
        "http",
        "https",
        "os",
        "path",
        "process",
        "punycode",
        "querystring",
        "stream",
        "string_decoder",
        "sys",
        "timers",
        "tty",
        "url",
        "util",
        "vm",
        "zlib",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Config {
    pub fn config_node_polyfill(config: &mut Config) {
        let builtins = get_node_builtins();

        for name in builtins.iter() {
            config
                .resolve
                .alias
                // why replace / ?
                // since a/b is not a valid js variable name
                .insert(
                    name.to_string(),
                    format!("node-libs-browser-okam/polyfill/{}", name),
                );
        }
    }
}
