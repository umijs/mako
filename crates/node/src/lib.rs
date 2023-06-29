#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::Arc;

use mako::compiler::Compiler;
use mako::config::Config;
use mako::dev::DevServer;
use mako::logger::init_logger;

#[napi]
pub async fn build(
    root: String,
    #[napi(ts_arg_type = r#"
{
    entry?: Record<string, string>;
    output?: {path: string};
    resolve?: {
       alias?: Record<string, string>;
       extensions?: string[];
    };
    mode?: "development" | "production";
    define?: Record<string, string>;
    devtool?: "source-map" | "inline-source-map";
    externals?: Record<string, string>;
    copy?: string[];
    providers?: Record<string, string[]>;
    public_path?: string;
    inline_limit?: number;
    targets?: Record<string, number>;
    platform?: "node" | "browser";
    hmr?: boolean;
    hmr_port?: string;
    hmr_host?: string;
}"#)]
    config: serde_json::Value,
    watch: bool,
) {
    // logger
    init_logger();

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let mako_config = Config::new(&root, Some(&default_config), None);

    match mako_config {
        Ok(config) => {
            let compiler = Compiler::new(config, root.clone());
            compiler.compile();

            if watch {
                let d = DevServer::new(root.clone(), Arc::new(compiler));
                // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
                d.serve().await;
            }
        }
        Err(e) => println!("error: {:?}", e),
    }
}
