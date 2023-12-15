use std::path::{Path, PathBuf};
use std::str::from_utf8_unchecked;
use std::sync::mpsc;
use std::sync::mpsc::Sender;

use mako::plugin::Plugin;
use mako_core::anyhow;
use napi::bindgen_prelude::*;
use napi::{JsObject, JsUndefined, JsUnknown, NapiRaw};

use crate::threadsafe_function;

pub struct WriteRequest {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub tx: Sender<Result<()>>,
}

pub fn create_fs_write_plugin(env: &Env, on_write: Option<JsFunction>) -> Option<FsWritePlugin> {
    if let Some(js_fs_write) = on_write {
        let write = threadsafe_function::ThreadsafeFunction::create(
            env.raw(),
            unsafe { js_fs_write.raw() },
            0,
            |ctx: threadsafe_function::ThreadSafeCallContext<WriteRequest>| unsafe {
                let path_str = ctx.value.path.to_str().unwrap();

                let str = ctx.env.create_string(path_str)?;
                let buffer = ctx
                    .env
                    .create_string(from_utf8_unchecked(&ctx.value.content))?;
                let result = ctx.callback.unwrap().call(None, &[str, buffer])?;
                await_promise(ctx.env, result, ctx.value.tx).unwrap();

                Ok(())
            },
        )
        .unwrap();

        Some(FsWritePlugin { write })
    } else {
        None
    }
}

pub struct FsWritePlugin {
    pub write: threadsafe_function::ThreadsafeFunction<WriteRequest>,
}

fn write<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    _content: C,
    js_fs_write: &threadsafe_function::ThreadsafeFunction<WriteRequest>,
) -> napi::Result<()> {
    let (tx, rx) = mpsc::channel::<napi::Result<()>>();
    js_fs_write.call(
        WriteRequest {
            path: path.as_ref().to_path_buf(),
            content: _content.as_ref().to_vec(),
            tx,
        },
        threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
    );
    if let Ok(r) = rx.recv() {
        r
    } else {
        Err(Error::from_status(napi::Status::GenericFailure))
    }
}

impl Plugin for FsWritePlugin {
    fn name(&self) -> &str {
        "fs_write_hook"
    }

    fn before_write_fs(&self, _path: &Path, _content: &[u8]) -> anyhow::Result<()> {
        write(_path, _content, &self.write)?;
        Ok(())
    }
}
// maybe we need a macro
fn await_promise(env: Env, result: JsUnknown, tx: Sender<napi::Result<()>>) -> napi::Result<()> {
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
        let _result: JsUndefined = result.try_into()?;
        tx.send(Ok(())).unwrap();
    }

    Ok(())
}
