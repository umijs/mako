use std::path::PathBuf;
use std::str::from_utf8_unchecked;
use std::sync::mpsc::Sender;

use mako::plugin::PluginGenerateEndParams;
use napi::bindgen_prelude::*;
use napi::{JsObject, JsString, JsUnknown, NapiRaw};

use crate::threadsafe_function;

#[napi(object)]
pub struct JsHooks {
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
    pub _on_generate_file: Option<JsFunction>,
    #[napi(ts_type = "() => Promise<void>;")]
    pub build_start: Option<JsFunction>,
}

pub struct TsFnHooks {
    pub build_start: Option<threadsafe_function::ThreadsafeFunction<ReadMessage<(), ()>>>,
    pub generate_end:
        Option<threadsafe_function::ThreadsafeFunction<ReadMessage<PluginGenerateEndParams, ()>>>,
    pub load:
        Option<threadsafe_function::ThreadsafeFunction<ReadMessage<String, Option<LoadResult>>>>,
    pub _on_generate_file: Option<threadsafe_function::ThreadsafeFunction<WriteRequest>>,
}

impl TsFnHooks {
    pub fn new(env: Env, hooks: &JsHooks) -> Self {
        Self {
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
            _on_generate_file: hooks._on_generate_file.as_ref().map(|hook| {
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

#[allow(dead_code)]
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

pub struct WriteRequest {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub tx: Sender<Result<()>>,
}

pub struct LoadResult {
    pub content: String,
    pub content_type: String,
}
