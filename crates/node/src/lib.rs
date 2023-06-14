#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use mako::compiler::Compiler;
use mako::config::Config;
use mako::logger::init_logger;

#[napi]
pub fn build(
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
    devtool?: "source-map" | "inline-source-map";
    externals?: Record<string, string>;
    copy?: string[];
    public_path?: string;
    data_url_limit?: number;
    targets?: Record<string, number>;
    platform?: "node" | "browser";
}"#)]
    config: serde_json::Value,
) {
    // logger
    init_logger();

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let mako_config = Config::new(&root, Some(&default_config), None);

    match mako_config {
        Ok(config) => {
            Compiler::new(config, root).compile();
        }
        Err(e) => println!("error: {:?}", e),
    }
}
