#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Once};
use std::time::UNIX_EPOCH;

use mako::compiler::{Args, Compiler, Context};
use mako::config::{Config, Mode};
use mako::dev::{DevServer, OnDevCompleteParams, Stats};
use mako::load::Content;
use mako::logger::init_logger;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{JsObject, JsString, JsUnknown, NapiRaw, Status};

mod threadsafe_function;

static LOG_INIT: Once = Once::new();

#[napi(object)]
pub struct JsHooks {
    pub on_compile_less: JsFunction,
}

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
    optimizePackageImports?: boolean;
}"#)]
    config: serde_json::Value,
    callback: JsFunction,
    js_hooks: JsHooks,
    watch: bool,
) -> napi::Result<JsObject> {
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

    let on_compile_less = js_hooks.on_compile_less;
    let on_compile_less = threadsafe_function::ThreadsafeFunction::create(
        e.raw(),
        unsafe { on_compile_less.raw() },
        0,
        |ctx: threadsafe_function::ThreadSafeCallContext<ReadMessage>| {
            let str = ctx.env.create_string(&ctx.value.message)?;
            let result = ctx.callback.unwrap().call(None, &[str])?;
            await_promise(ctx.env, result, ctx.value.tx).unwrap();
            Ok(())
        },
    )?;
    let less_plugin = LessPlugin { on_compile_less };
    let extra_plugins: Vec<Arc<dyn Plugin>> = vec![Arc::new(less_plugin)];

    let default_config = serde_json::to_string(&config).unwrap();
    let root = std::path::PathBuf::from(&root);
    let mut config = Config::new(&root, Some(&default_config), None)
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;

    // dev 环境下不产生 hash, prod 环境下根据用户配置
    if config.mode == Mode::Development {
        config.hash = false;
    }

    if watch {
        let (deferred, promise) = e.create_deferred()?;
        e.execute_tokio_future(
            async move {
                let compiler =
                    Compiler::new(config, root.clone(), Args { watch }, Some(extra_plugins))
                        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
                compiler
                    .compile()
                    .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
                let d = DevServer::new(root.clone(), Arc::new(compiler));
                deferred.resolve(move |env| env.get_undefined());
                d.serve(move |params| {
                    callback.call(Ok(params), ThreadsafeFunctionCallMode::Blocking);
                })
                .await;
                Ok(())
            },
            move |&mut _, _res| Ok(()),
        )?;
        Ok(promise)
    } else {
        let (deferred, promise) = e.create_deferred()?;
        mako_core::rayon::spawn(move || {
            let start_time = std::time::SystemTime::now();
            let compiler = Compiler::new(config, root.clone(), Args { watch }, Some(extra_plugins))
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
            let end_time = std::time::SystemTime::now();
            callback.call(
                Ok(OnDevCompleteParams {
                    is_first_compile: true,
                    time: end_time.duration_since(start_time).unwrap().as_millis() as u64,
                    stats: Stats {
                        start_time: start_time.duration_since(UNIX_EPOCH).unwrap().as_millis()
                            as u64,
                        end_time: end_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                    },
                }),
                ThreadsafeFunctionCallMode::Blocking,
            );
            deferred.resolve(move |env| env.get_undefined());
        });
        Ok(promise)
    }
}

fn await_promise(
    env: Env,
    result: JsUnknown,
    tx: Sender<napi::Result<String>>,
) -> napi::Result<()> {
    // If the result is a promise, wait for it to resolve, and send the result to the channel.
    // Otherwise, send the result immediately.
    if result.is_promise()? {
        let result: JsObject = result.try_into()?;
        let then: JsFunction = result.get_named_property("then")?;
        let tx2 = tx.clone();
        let cb = env.create_function_from_closure("callback", move |ctx| {
            let res = ctx.get::<JsString>(0)?.into_utf8()?;
            let s = res.into_owned()?;
            tx.send(Ok(s)).unwrap();
            ctx.env.get_undefined()
        })?;
        let eb = env.create_function_from_closure("error_callback", move |ctx| {
            let res = ctx.get::<JsUnknown>(0)?;
            tx2.send(Err(napi::Error::from(res))).unwrap();
            ctx.env.get_undefined()
        })?;
        then.call(Some(&result), &[cb, eb])?;
    } else {
        let result: JsString = result.try_into()?;
        let utf8 = result.into_utf8()?;
        let s = utf8.into_owned()?;
        tx.send(Ok(s)).unwrap();
    }

    Ok(())
}

pub struct ReadMessage {
    pub message: String,
    pub tx: Sender<Result<String>>,
}

pub struct LessPlugin {
    pub on_compile_less: threadsafe_function::ThreadsafeFunction<ReadMessage>,
}
use mako::plugin::{Plugin, PluginLoadParam};

impl LessPlugin {
    fn compile_less(&self, path: &str) -> Result<String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<String>>();
        self.on_compile_less.call(
            ReadMessage {
                message: path.to_string(),
                tx,
            },
            threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
        );

        rx.recv()
            .unwrap_or_else(|e| panic!("recv error: {:?}", e.to_string()))
    }
}

impl Plugin for LessPlugin {
    fn name(&self) -> &str {
        "less"
    }

    fn load(
        &self,
        param: &PluginLoadParam,
        _context: &Arc<Context>,
    ) -> mako_core::anyhow::Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "less") {
            let content = self.compile_less(param.path.as_str())?;
            return Ok(Some(Content::Css(content)));
        }
        Ok(None)
    }
}
