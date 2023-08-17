#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::Arc;

use mako::compiler::{CompileOptions, Compiler};
use mako::config::Config;
use mako::dev::DevServer;
use mako::logger::init_logger;

#[napi]
pub async fn build(
    root: String,
    #[napi(ts_arg_type = r#"
{
    entry?: Record<string, string>;
    output?: {path: string; mode: "bundle" | "minifish" ;  esVersion?: string, };
    resolve?: {
       alias?: Record<string, string>;
       extensions?: string[];
    };
    manifest?: boolean;
    manifest_config?: {
        file_name: string;
        base_path: string;
    };
    mode?: "development" | "production";
    define?: Record<string, string>;
    devtool?: "source-map" | "inline-source-map" | "none";
    externals?: Record<string, string>;
    copy?: string[];
    codeSplitting: "bigVendor" | "depPerChunk" | "none";
    providers?: Record<string, string[]>;
    public_path?: string;
    inline_limit?: number;
    targets?: Record<string, number>;
    platform?: "node" | "browser";
    hmr?: boolean;
    hmr_port?: string;
    hmr_host?: string;
    stats?: boolean;
}"#)]
    config: serde_json::Value,
    watch: bool,
) {
    // logger
    init_logger();

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let config = Config::new(&root, Some(&default_config), None).unwrap();

    let compiler = Compiler::new(config, root.clone());
    compiler.compile(Some(CompileOptions { watch }));
    if watch {
        let d = DevServer::new(root.clone(), Arc::new(compiler));
        // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
        d.serve().await;
    }
}
