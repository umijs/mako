use crate::config::Config;

fn get_node_builtins() -> Vec<String> {
    vec![
        "assert",
        "assert/strict",
        "async_hooks",
        "buffer",
        "child_process",
        "cluster",
        "console",
        "constants",
        "crypto",
        "dgram",
        "diagnostics_channel",
        "dns",
        "dns/promises",
        "domain",
        "events",
        "fs",
        "fs/promises",
        "http",
        "http2",
        "https",
        "inspector",
        "inspector/promises",
        "module",
        "net",
        "os",
        "path",
        "path/posix",
        "path/win32",
        "perf_hooks",
        "process",
        "punycode",
        "querystring",
        "readline",
        "readline/promises",
        "repl",
        "stream",
        "stream/consumers",
        "stream/promises",
        "stream/web",
        "string_decoder",
        "sys",
        "timers",
        "timers/promises",
        "tls",
        "trace_events",
        "tty",
        "url",
        "util",
        "util/types",
        "v8",
        "vm",
        "wasi",
        "worker_threads",
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
                .externals
                // why replace / ?
                // since a/b is not a valid js variable name
                .insert(name.to_string(), name.to_string().replace('/', "__"));
        }
    }
}
