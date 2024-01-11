#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::path::PathBuf;
use std::str::from_utf8_unchecked;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Once};

use js_plugin::JsPlugin;
use mako::compiler::{Args, Compiler};
use mako::config::Config;
use mako::dev::DevServer;
use mako::logger::init_logger;
use mako::plugin::{Plugin, PluginGenerateEndParams};
use napi::bindgen_prelude::*;
use napi::{JsObject, JsString, JsUnknown, NapiRaw, Status};

mod js_plugin;

pub(crate) mod threadsafe_function;

static LOG_INIT: Once = Once::new();

#[napi(object)]
pub struct JsHooks {
    #[napi(ts_type = "(filePath: string) => Promise<string> ;")]
    pub on_compile_less: Option<JsFunction>,
    #[napi(
        ts_type = "(filePath: string) => Promise<{ content: string, type: 'css'|'javascript' }> ;"
    )]
    pub load: Option<JsFunction>,
    #[napi(ts_type = "(data: {isFirstCompile: boolean; time: number; stats: {
        startTime: number;
        endTime: number;
    }}) =>void ;")]
    pub generate_end: Option<JsFunction>,
    #[napi(ts_type = "(path: string, content: Buffer) => Promise<void>;")]
    pub on_generate_file: Option<JsFunction>,
    #[napi(ts_type = "() => Promise<void>;")]
    pub build_start: Option<JsFunction>,
}

pub struct WriteRequest {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub tx: Sender<Result<()>>,
}

pub struct TsFnHooks {
    pub build_start: Option<threadsafe_function::ThreadsafeFunction<ReadMessage<(), ()>>>,
    pub on_compile_less:
        Option<threadsafe_function::ThreadsafeFunction<ReadMessage<String, String>>>,
    pub generate_end:
        Option<threadsafe_function::ThreadsafeFunction<ReadMessage<PluginGenerateEndParams, ()>>>,
    pub on_generate_file: Option<threadsafe_function::ThreadsafeFunction<WriteRequest>>,
    pub load:
        Option<threadsafe_function::ThreadsafeFunction<ReadMessage<String, Option<LoadResult>>>>,
}

pub struct LoadResult {
    pub content: String,
    pub content_type: String,
}

impl TsFnHooks {
    fn new(env: Env, hooks: &JsHooks) -> Self {
        Self {
            load: hooks.load.as_ref().map(|hook| {
                threadsafe_function::ThreadsafeFunction::create(
                    env.raw(),
                    unsafe { hook.raw() },
                    0,
                    |ctx: threadsafe_function::ThreadSafeCallContext<
                        ReadMessage<String, Option<LoadResult>>,
                    >| {
                        let str = ctx.env.create_string(&ctx.value.message)?;
                        let result = ctx.callback.unwrap().call(None, &[str])?;
                        await_promise_js_object(ctx.env, result, ctx.value.tx).unwrap();
                        Ok(())
                    },
                )
                .unwrap()
            }),
            build_start: hooks.build_start.as_ref().map(|hook| {
                threadsafe_function::ThreadsafeFunction::create(
                    env.raw(),
                    unsafe { hook.raw() },
                    0,
                    |ctx: threadsafe_function::ThreadSafeCallContext<ReadMessage<(), ()>>| {
                        let obj = ctx.env.create_object()?;
                        let result = ctx.callback.unwrap().call(None, &[obj])?;
                        await_promise_with_void(ctx.env, result, ctx.value.tx).unwrap();
                        Ok(())
                    },
                )
                .unwrap()
            }),
            on_compile_less: hooks.on_compile_less.as_ref().map(|hook| {
                threadsafe_function::ThreadsafeFunction::create(
                    env.raw(),
                    unsafe { hook.raw() },
                    0,
                    |ctx: threadsafe_function::ThreadSafeCallContext<
                        ReadMessage<String, String>,
                    >| {
                        let str = ctx.env.create_string(&ctx.value.message)?;
                        let result = ctx.callback.unwrap().call(None, &[str])?;
                        await_promise(ctx.env, result, ctx.value.tx).unwrap();
                        Ok(())
                    },
                )
                .unwrap()
            }),
            generate_end: hooks.generate_end.as_ref().map(|hook| {
                threadsafe_function::ThreadsafeFunction::create(
                    env.raw(),
                    unsafe { hook.raw() },
                    0,
                    |ctx: threadsafe_function::ThreadSafeCallContext<
                        ReadMessage<PluginGenerateEndParams, ()>,
                    >| {
                        let mut obj = ctx.env.create_object()?;
                        let mut stats = ctx.env.create_object()?;
                        stats.set_named_property(
                            "startTime",
                            ctx.env
                                .create_int64(ctx.value.message.stats.start_time as i64)?,
                        )?;
                        stats.set_named_property(
                            "endTime",
                            ctx.env
                                .create_int64(ctx.value.message.stats.end_time as i64)?,
                        )?;
                        obj.set_named_property(
                            "isFirstCompile",
                            ctx.value.message.is_first_compile,
                        )?;
                        obj.set_named_property(
                            "time",
                            ctx.env.create_int64(ctx.value.message.time as i64),
                        )?;
                        obj.set_named_property("stats", stats)?;
                        let result = ctx.callback.unwrap().call(None, &[obj])?;
                        await_promise_with_void(ctx.env, result, ctx.value.tx).unwrap();
                        Ok(())
                    },
                )
                .unwrap()
            }),
            on_generate_file: hooks.on_generate_file.as_ref().map(|hook| {
                threadsafe_function::ThreadsafeFunction::create(
                    env.raw(),
                    unsafe { hook.raw() },
                    0,
                    |ctx: threadsafe_function::ThreadSafeCallContext<WriteRequest>| unsafe {
                        let path_str = ctx.value.path.to_str().unwrap();
                        let str = ctx.env.create_string(path_str)?;
                        let buffer = ctx
                            .env
                            .create_string(from_utf8_unchecked(&ctx.value.content))?;
                        let result = ctx.callback.unwrap().call(None, &[str, buffer])?;
                        await_promise_with_void(ctx.env, result, ctx.value.tx).unwrap();
                        Ok(())
                    },
                )
                .unwrap()
            }),
        }
    }
}

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
    transformImport?: { libraryName: string; libraryDirectory?: string; style?: boolean | string }[];
    clean?: boolean;
    nodePolyfill?: boolean;
    ignores?: string[];
    moduleIdStrategy?: "hashed" | "named";
    minify?: boolean;
    _minifish?: false | {
        mapping: Record<string, string>;
        metaPath?: string;
        inject?: Record<string, { from:string;exclude?:string; preferRequire?: boolean } |
            { from:string; named:string; exclude?:string; preferRequire?: boolean } |
            { from:string; namespace: true; exclude?:string; preferRequire?: boolean }
            >;
    };
    optimization?: false | {
        skipModules?: boolean;
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

    // if let Some(plugin) = create_fs_write_plugin(&env, build_params.hooks.on_generate_file) {
    //     plugins.push(Arc::new(plugin));
    // }

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

fn await_promise_js_object(
    env: Env,
    result: JsUnknown,
    tx: Sender<napi::Result<Option<LoadResult>>>,
) -> napi::Result<()> {
    // If the result is a promise, wait for it to resolve, and send the result to the channel.
    // Otherwise, send the result immediately.
    if result.is_promise()? {
        let result: JsObject = result.try_into()?;
        let then: JsFunction = result.get_named_property("then")?;
        let tx2 = tx.clone();
        let cb = env.create_function_from_closure("callback", move |ctx| {
            let res = ctx.get::<JsUnknown>(0)?;
            if matches!(res.get_type()?, ValueType::Undefined) {
                tx.send(Ok(None)).unwrap();
                return ctx.env.get_undefined();
            }
            let res: JsObject = res.try_into()?;
            // let s = res.into_owned()?;
            // get res.content as string
            // println!("res: {:}", res.get_element(0)?.into_utf8()?.into_owned()?);
            let content: JsString = res.get_named_property("content")?;
            let content_type: JsString = res.get_named_property("type")?;
            tx.send(Ok(Some(LoadResult {
                content: content.into_utf8()?.into_owned()?,
                content_type: content_type.into_utf8()?.into_owned()?,
            })))
            .unwrap();
            ctx.env.get_undefined()
        })?;
        let eb = env.create_function_from_closure("error_callback", move |ctx| {
            let res = ctx.get::<JsUnknown>(0)?;
            tx2.send(Err(napi::Error::from(res))).unwrap();
            ctx.env.get_undefined()
        })?;
        then.call(Some(&result), &[cb, eb])?;
    } else {
        if matches!(result.get_type()?, ValueType::Undefined) {
            tx.send(Ok(None)).unwrap();
            return Ok(());
        }
        let res: JsObject = result.try_into()?;
        let content: JsString = res.get_named_property("content")?;
        let content_type: JsString = res.get_named_property("type")?;
        tx.send(Ok(Some(LoadResult {
            content: content.into_utf8()?.into_owned()?,
            content_type: content_type.into_utf8()?.into_owned()?,
        })))
        .unwrap();
    }

    Ok(())
}

fn await_promise_with_void(
    env: Env,
    result: JsUnknown,
    tx: Sender<napi::Result<()>>,
) -> napi::Result<()> {
    // If the result is a promise, wait for it to resolve, and send the result to the channel.
    // Otherwise, send the result immediately.
    if result.is_promise()? {
        let result: JsObject = result.try_into()?;
        let then: JsFunction = result.get_named_property("then")?;
        let tx2 = tx.clone();
        let cb = env.create_function_from_closure("callback", move |ctx| {
            tx.send(Ok(())).unwrap();
            ctx.env.get_undefined()
        })?;
        let eb = env.create_function_from_closure("error_callback", move |ctx| {
            let res = ctx.get::<JsUnknown>(0)?;
            tx2.send(Err(napi::Error::from(res))).unwrap();
            ctx.env.get_undefined()
        })?;
        then.call(Some(&result), &[cb, eb])?;
    } else {
        tx.send(Ok(())).unwrap();
    }

    Ok(())
}

pub struct ReadMessage<T, V> {
    pub message: T,
    pub tx: Sender<Result<V>>,
}
