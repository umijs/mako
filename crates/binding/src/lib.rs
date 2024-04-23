#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::{Arc, Once};

use js_plugin::JsPlugin;
use mako::compiler::{Args, Compiler};
use mako::config::Config;
use mako::dev::DevServer;
use mako::logger::init_logger;
use mako::plugin::Plugin;
use napi::bindgen_prelude::*;
use napi::{JsObject, Status};
use tsfn::{JsHooks, TsFnHooks};

mod js_plugin;
mod tsfn;

pub(crate) mod threadsafe_function;

#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: mimalloc_rust::GlobalMiMalloc = mimalloc_rust::GlobalMiMalloc;

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

static LOG_INIT: Once = Once::new();

#[napi(object)]
pub struct BuildParams {
    pub root: String,

    #[napi(ts_type = r#"
{
    entry?: Record<string, string>;
    output?: {
        path: string;
        mode: "bundle" | "bundless" ;
        esVersion?: string;
        meta?: boolean;
        preserveModules?: boolean;
        preserveModulesRoot?: string;
        asciiOnly?: boolean;
        skipWrite?: boolean;
    };
    resolve?: {
       alias?: Record<string, string>;
       extensions?: string[];
    };
    manifest?: false | {
        fileName: string;
        basePath: string;
    };
    mode?: "development" | "production";
    define?: Record<string, string>;
    devtool?: false | "source-map" | "inline-source-map";
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
    codeSplitting?: false | "auto";
    providers?: Record<string, string[]>;
    publicPath?: string;
    inlineLimit?: number;
    targets?: Record<string, number>;
    platform?: "node" | "browser";
    hmr?: false | { host?: string; port?: number };
    px2rem?: false | {
        root?: number;
        propBlackList?: string[];
        propWhiteList?: string[];
        selectorBlackList?: string[];
        selectorWhiteList?: string[];
    };
    stats?: boolean;
    hash?: boolean;
    autoCSSModules?: boolean;
    ignoreCSSParserErrors?: boolean;
    dynamicImportToRequire?: boolean;
    umd?: false | string;
    cjs?: boolean;
    writeToDisk?: boolean;
    transformImport?: { libraryName: string; libraryDirectory?: string; style?: boolean | string }[];
    clean?: boolean;
    nodePolyfill?: boolean;
    ignores?: string[];
    moduleIdStrategy?: "hashed" | "named";
    minify?: boolean;
    _minifish?: false | {
        mapping: Record<string, string>;
        metaPath?: string;
        inject?: Record<string, { from:string;exclude?:string; include?:string; preferRequire?:
        boolean } |
            { from:string; named:string; exclude?:string; include?:string;preferRequire?: boolean
             } |
            { from:string; namespace: true; exclude?:string; include?:string; preferRequire?:
            boolean }
            >;
    };
    optimization?: false | {
        skipModules?: boolean;
    };
    react?: {
        runtime?: "automatic" | "classic";
        pragma?: string;
        importSource?: string;
        pragmaFrag?: string;
    };
    emitAssets?: boolean;
    cssModulesExportOnlyLocales?: boolean;
    inlineCSS?: false | {};
    rscServer?: false | {};
    rscClient?: false | {};
    experimental?: {
        webpackSyntaxValidate?: string[];
    };
}"#)]
    pub config: serde_json::Value,
    pub hooks: JsHooks,
    pub watch: bool,
}

#[napi(ts_return_type = r#"Promise<void>"#)]
pub fn build(env: Env, build_params: BuildParams) -> napi::Result<JsObject> {
    LOG_INIT.call_once(|| {
        init_logger();
    });

    let mut plugins: Vec<Arc<dyn Plugin>> = vec![];
    let tsfn_hooks = TsFnHooks::new(env, &build_params.hooks);
    let plugin = JsPlugin { hooks: tsfn_hooks };
    plugins.push(Arc::new(plugin));

    let root = std::path::PathBuf::from(&build_params.root);
    let default_config = serde_json::to_string(&build_params.config).unwrap();
    let config = Config::new(&root, Some(&default_config), None).map_err(|e| {
        napi::Error::new(Status::GenericFailure, format!("Load config failed: {}", e))
    })?;

    if build_params.watch {
        let (deferred, promise) = env.create_deferred()?;
        env.execute_tokio_future(
            async move {
                let compiler =
                    Compiler::new(config, root.clone(), Args { watch: true }, Some(plugins))
                        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)));
                if let Err(e) = compiler {
                    deferred.reject(e);
                    return Ok(());
                }
                let compiler = compiler.unwrap();

                if let Err(e) = compiler
                    .compile()
                    .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))
                {
                    deferred.reject(e);
                    return Ok(());
                }
                let d = DevServer::new(root.clone(), Arc::new(compiler));
                deferred.resolve(move |env| env.get_undefined());
                d.serve(move |_params| {}).await;
                Ok(())
            },
            move |&mut _, _res| Ok(()),
        )?;
        Ok(promise)
    } else {
        let (deferred, promise) = env.create_deferred()?;
        mako_core::rayon::spawn(move || {
            let compiler =
                Compiler::new(config, root.clone(), Args { watch: false }, Some(plugins))
                    .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)));
            let compiler = match compiler {
                Ok(c) => c,
                Err(e) => {
                    deferred.reject(e);
                    return;
                }
            };
            let ret = compiler
                .compile()
                .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)));
            if let Err(e) = ret {
                deferred.reject(e);
                return;
            }
            deferred.resolve(move |env| env.get_undefined());
        });
        Ok(promise)
    }
}
