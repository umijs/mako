use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pathdiff::diff_paths;
use serde_json::Value;
use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{Expr, Lit, Str};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::config::{Config, ExternalConfig, Platform};

pub struct Node {}

impl Node {
    pub fn modify_config(config: &mut Config) {
        if config.platform == Platform::Node {
            // set default node target
            let target = config.targets.get("node").unwrap_or(&14.0);
            config.targets = HashMap::from([("node".into(), *target)]);
            // ignore all built-in node modules
            config.ignores.push(format!(
                "^(node:)?({})(/.+|$)",
                Self::get_all_node_modules().join("|")
            ));
            // polifyll __dirname & __filename is supported with MockFilenameAndDirname Visitor
        } else {
            // polyfill __dirname & __filename for browser
            config
                .define
                .insert("__dirname".into(), Value::String("'/'".into()));
            config
                .define
                .insert("__filename".into(), Value::String("'/index.js'".into()));
            // polyfill with equivalent modules
            for name in Self::get_polyfill_modules().iter() {
                config.resolve.alias.push((
                    name.to_string(),
                    format!("node-libs-browser-okam/polyfill/{}", name),
                ));
            }
            // polyfill with empty modules
            for name in Self::get_empty_modules().iter() {
                // e.g. support fs and fs/promise
                config
                    .externals
                    .insert(name.to_string(), ExternalConfig::Basic("".to_string()));
                config.externals.insert(
                    format!("{name}/promise"),
                    ExternalConfig::Basic("".to_string()),
                );
            }
            // polyfill identifiers
            config
                .providers
                .insert("process".into(), ("process".into(), "".into()));
            config
                .providers
                .insert("Buffer".into(), ("buffer".into(), "Buffer".into()));
            config.providers.insert(
                "global".into(),
                ("node-libs-browser-okam/polyfill/global".into(), "".into()),
            );
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
        [
            "async_hooks",
            "child_process",
            "cluster",
            "dgram",
            "diagnostics_channel",
            "dns",
            "fs",
            "http2",
            "inspector",
            "module",
            "net",
            "perf_hooks",
            "readline",
            "repl",
            "tls",
            "trace_events",
            "v8",
            "wasi",
            "worker_threads",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn get_all_node_modules() -> Vec<String> {
        let mut modules = Self::get_polyfill_modules();
        modules.extend(Self::get_empty_modules());
        modules
    }
}

pub struct MockFilenameAndDirname {
    pub unresolved_mark: Mark,
    pub current_path: PathBuf,
    pub context: Arc<Context>,
}

impl VisitMut for MockFilenameAndDirname {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Ident(ident) = expr
            && ident.ctxt.outer() == self.unresolved_mark
        {
            let is_filename = ident.sym == "__filename";
            let is_dirname = ident.sym == "__dirname";
            if is_filename || is_dirname {
                let path = diff_paths(&self.current_path, &self.context.root).unwrap_or("".into());
                let value = if is_filename {
                    path
                } else {
                    path.parent().unwrap_or(&PathBuf::from("")).into()
                };

                *expr = Expr::Lit(Lit::Str(Str {
                    span: DUMMY_SP,
                    value: value.to_string_lossy().into(),
                    raw: None,
                }));
            }
        }

        expr.visit_mut_children_with(self);
    }
}
