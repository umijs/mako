#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use mako::compiler::Compiler;
use mako::config::Config;

#[napi]
pub fn build(
    root: String,
    #[napi(ts_arg_type = r#"
{
    entry: Record<string, string>;
    output: {path: string};
    resolve: {
       alias: Record<string, string>;
       extensions: string[];
    };
    mode: "development" | "production";
    sourcemap: boolean | "inline";
    externals: Record<string, string>;
    copy: string[];
    public_path: string;
    data_url_limit: number;
}"#)]
    config: serde_json::Value,
) {
    let mako_config = serde_json::from_value::<Config>(config);

    match mako_config {
        Ok(config) => {
            Compiler::new(config, root.into()).compile();
        }
        Err(e) => println!("error: {:?}", e),
    }
}
