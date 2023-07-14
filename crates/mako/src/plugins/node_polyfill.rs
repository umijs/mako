use anyhow::Result;

use crate::config::Config;
use crate::plugin::Plugin;

pub struct NodePolyfillPlugin {}

impl Plugin for NodePolyfillPlugin {
    fn name(&self) -> &str {
        "node_polyfill"
    }

    fn modify_config(&self, config: &mut Config) -> Result<()> {
        // polyfill modules
        for name in get_polyfill_modules().iter() {
            config.resolve.alias.insert(
                name.to_string(),
                format!("node-libs-browser-okam/polyfill/{}", name),
            );
        }
        // empty modules
        for name in get_empty_modules().iter() {
            config.externals.insert(name.to_string(), "".to_string());
        }
        // identifier
        config
            .providers
            .insert("process".into(), ("process".into(), "".into()));
        config
            .providers
            .insert("Buffer".into(), ("buffer".into(), "Buffer".into()));
        Ok(())
    }
}

fn get_polyfill_modules() -> Vec<String> {
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

fn get_empty_modules() -> Vec<String> {
    vec![
        "child_process",
        "cluster",
        "dgram",
        "dns",
        "fs",
        "module",
        "net",
        "readline",
        "repl",
        "tls",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
