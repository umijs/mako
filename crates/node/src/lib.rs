#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Once};
use std::time::UNIX_EPOCH;

use mako::compiler::{Args, Compiler};
use mako::config::Config;
use mako::dev::{DevServer, OnDevCompleteParams, Stats};
use mako::logger::init_logger;
use mako::plugin::Plugin;
use napi::bindgen_prelude::*;
use napi::{JsObject, JsString, JsUnknown, NapiRaw, Status};

mod plugin_less;
mod threadsafe_function;

static LOG_INIT: Once = Once::new();

#[napi(object)]
pub struct JsHooks {
    pub on_compile_less: Option<JsFunction>,
    pub on_build_complete: Option<JsFunction>,
}

#[napi(object)]
pub struct BuildParams {
    pub root: String,
    pub config: serde_json::Value,
    pub hooks: JsHooks,
    pub watch: bool,
}

fn call_on_build_complete(
    on_build_complete: &Option<threadsafe_function::ThreadsafeFunction<OnDevCompleteParams>>,
    params: OnDevCompleteParams,
) {
    if let Some(func) = on_build_complete {
        func.call(
            params,
            threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking,
        );
    }
}

#[napi]
pub fn build(env: Env, build_params: BuildParams) -> napi::Result<JsObject> {
    LOG_INIT.call_once(|| {
        init_logger();
    });

    let on_build_complete = if let Some(on_build_complete) = build_params.hooks.on_build_complete {
        let func = threadsafe_function::ThreadsafeFunction::create(
            env.raw(),
            unsafe { on_build_complete.raw() },
            0,
            |ctx: threadsafe_function::ThreadSafeCallContext<OnDevCompleteParams>| {
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
                ctx.callback.unwrap().call(None, &[obj])?;
                Ok(())
            },
        )?;
        Some(func)
    } else {
        None
    };

    let less_plugin = if let Some(on_compile_less) = build_params.hooks.on_compile_less {
        let on_compile_less = threadsafe_function::ThreadsafeFunction::create(
            env.raw(),
            unsafe { on_compile_less.raw() },
            0,
            |ctx: threadsafe_function::ThreadSafeCallContext<ReadMessage>| {
                let str = ctx.env.create_string(&ctx.value.message)?;
                let result = ctx.callback.unwrap().call(None, &[str])?;
                await_promise(ctx.env, result, ctx.value.tx).unwrap();
                Ok(())
            },
        )?;
        Some(Arc::new(plugin_less::LessPlugin { on_compile_less }))
    } else {
        None
    };

    let mut plugins: Vec<Arc<dyn Plugin>> = vec![];
    if let Some(less_plugin) = less_plugin {
        plugins.push(less_plugin);
    }

    let root = std::path::PathBuf::from(&build_params.root);
    let default_config = serde_json::to_string(&build_params.config).unwrap();
    let config = Config::new(&root, Some(&default_config), None)
        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;

    if build_params.watch {
        let (deferred, promise) = env.create_deferred()?;
        env.execute_tokio_future(
            async move {
                let start_time = std::time::SystemTime::now();
                let compiler =
                    Compiler::new(config, root.clone(), Args { watch: true }, Some(plugins))
                        .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
                compiler
                    .compile()
                    .map_err(|e| napi::Error::new(Status::GenericFailure, format!("{}", e)))?;
                let end_time = std::time::SystemTime::now();
                let params = OnDevCompleteParams {
                    is_first_compile: true,
                    time: end_time.duration_since(start_time).unwrap().as_millis() as u64,
                    stats: Stats {
                        start_time: start_time.duration_since(UNIX_EPOCH).unwrap().as_millis()
                            as u64,
                        end_time: end_time.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                    },
                };
                call_on_build_complete(&on_build_complete, params);
                let d = DevServer::new(root.clone(), Arc::new(compiler));
                deferred.resolve(move |env| env.get_undefined());
                d.serve(move |params| {
                    call_on_build_complete(&on_build_complete, params);
                })
                .await;
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

pub struct ReadMessage {
    pub message: String,
    pub tx: Sender<Result<String>>,
}
