#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::{Arc, Once};
use std::time::UNIX_EPOCH;

use mako::compiler::{Args, Compiler};
use mako::config::{Config, Mode};
use mako::dev::{DevServer, OnDevCompleteParams, Stats};
use mako::logger::init_logger;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::Status;
static LOG_INIT: Once = Once::new();

#[napi]
pub fn build(
    e: Env,
    root: String,
    #[napi(ts_arg_type = r#"
{
    entry?: Record<string, string>;
    output?: {
        path: string;
        mode: "bundle" | "bundless" ;
        esVersion?: string;
        meta?: boolean;
        asciiOnly?: boolean,
        preserveModules?: boolean;
        preserveModulesRoot?: string;
    };
    resolve?: {
       alias?: Record<string, string>;
       extensions?: string[];
    };
    manifest?: boolean;
    manifestConfig?: {
        fileName: string;
        basePath: string;
    };
    minify?: boolean;
    mode?: "development" | "production";
    define?: Record<string, string>;
    devtool?: "source-map" | "inline-source-map" | "none";
    externals?: Record<
        string,
        string | {
            root: string;
            script?: string;
            subpath?: {
                exclude?: string[];
                rules: {
                    regex: string;
                    target: string | '$EMPTY';
                    targetConverter?: 'PascalCase';
                }[];
            };
        }
    >;
    copy?: string[];
    code_splitting?: "auto" | "none";
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
    transformImport?: { libraryName: string; libraryDirectory?: string; style?: boolean | string }[];
    clean?: boolean;
    nodePolyfill?: boolean;
    ignores?: string[];
    _minifish?: {
        mapping: Record<string, string>;
        metaPath?: string;
        inject?: Record<string, { from:string;exclude?:string; } |
            { from:string; named:string; exclude?:string } |
            { from:string; namespace: true; exclude?:string }
            >;
    };
}"#)]
    config: serde_json::Value,
    callback: JsFunction,
    watch: bool,
) -> napi::Result<()> {
    // logger
    LOG_INIT.call_once(|| {
        init_logger();
    });

    let callback: ThreadsafeFunction<OnDevCompleteParams, ErrorStrategy::CalleeHandled> = callback
        .create_threadsafe_function(
            0,
            |ctx: napi::threadsafe_function::ThreadSafeCallContext<OnDevCompleteParams>| {
                let mut obj = ctx.env.create_object()?;
                let mut stats = ctx.env.create_object()?;
                stats.set_named_property(
                    "startTime",
                    ctx.env.create_int64(ctx.value.stats.start_time as i64)?,
                )?;
                stats.set_named_property(
                    "endTime",
                    ctx.env.create_int64(ctx.value.stats.end_time as i64)?,
                )?;
                obj.set_named_property("isFirstCompile", ctx.value.is_first_compile)?;
                obj.set_named_property("time", ctx.env.create_int64(ctx.value.time as i64))?;
                obj.set_named_property("stats", stats)?;
                Ok(vec![obj])
            },
        )?;

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let mut config = Config::new(&root, Some(&default_config), None)
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;

    // dev 环境下不产生 hash, prod 环境下根据用户配置
    if config.mode == Mode::Development {
        config.hash = false;
    }

    let compiler = Compiler::new(config, root.clone(), Args { watch })
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
    let start_time = std::time::SystemTime::now();
    compiler
        .compile()
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
    let end_time = std::time::SystemTime::now();
    callback.call(
        Ok(OnDevCompleteParams {
            is_first_compile: true,
            time: end_time.duration_since(start_time).unwrap().as_millis() as u64,
            stats: Stats {
                start_time: start_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                end_time: end_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            },
        }),
        ThreadsafeFunctionCallMode::Blocking,
    );
    if watch {
        e.execute_tokio_future(
            async move {
                let d = DevServer::new(root.clone(), Arc::new(compiler));
                // TODO: when in Dev Mode, Dev Server should start asap, and provider a loading  while in first compiling
                d.serve(move |params| {
                    callback.call(Ok(params), ThreadsafeFunctionCallMode::Blocking);
                })
                .await;
                Ok(())
            },
            move |&mut _, _res| Ok(()),
        )?;
    }
    Ok(())
}
