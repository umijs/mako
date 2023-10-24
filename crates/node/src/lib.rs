#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::Arc;

use mako::compiler::{Args, Compiler};
use mako::config::{Config, Mode};
use mako::dev::DevServer;
use mako::logger::init_logger;
use napi::Status;

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
    manifestConfig?: {
        fileName: string;
        basePath: string;
    };
    mode?: "development" | "production";
    define?: Record<string, string>;
    devtool?: "source-map" | "inline-source-map" | "none";
    externals?: Record<string, string>;
    copy?: string[];
    code_splitting: "auto" | "none";
    providers?: Record<string, string[]>;
    publicPath?: string;
    inlineLimit?: number;
    targets?: Record<string, number>;
    platform?: "node" | "browser";
    hmr?: boolean;
    hmrPort?: string;
    hmrHost?: string;
    px2rem?: boolean;
    px2remConfig?: {
        root: number;
        propBlackList: string[];
        propWhiteList: string[];
        selectorBlackList: string[];
        selectorWhiteList: string[];
    };
    stats?: boolean;
    hash?: boolean;
    autoCssModules?: boolean;
    ignoreCSSParserErrors?: boolean;
    dynamicImportToRequire?: boolean;
    umd?: string;
}"#)]
    config: serde_json::Value,
    watch: bool,
) -> napi::Result<()> {
    // logger
    init_logger();

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let mut config = Config::new(&root, Some(&default_config), None).unwrap();

    // dev 环境下不产生 hash, prod 环境下根据用户配置
    if config.mode == Mode::Development {
        config.hash = false;
    }

    let compiler = Compiler::new(config, root.clone(), Args { watch })
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
    compiler
        .compile()
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
    if watch {
        let d = DevServer::new(root.clone(), Arc::new(compiler));
        // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
        d.serve().await;
    }
    Ok(())
}
