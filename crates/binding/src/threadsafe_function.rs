use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use napi::bindgen_prelude::{spawn, FromNapiValue, JsValuesTupleIntoVec, Promise, ToNapiValue};
use napi::sys::{napi_env, napi_value};
use napi::threadsafe_function::{
    ErrorStrategy, ThreadsafeFunction as Tsfn, ThreadsafeFunctionCallMode,
};
use napi::{JsObject, JsUnknown};
use oneshot::channel;

use crate::context::create_js_context;

pub struct ThreadsafeFunction<P: 'static, R> {
    tsfn: Tsfn<Args<P>, ErrorStrategy::Fatal>,
    env: napi_env,
    _phantom: PhantomData<R>,
}

struct Args<P>(JsObject, P);

impl<P> JsValuesTupleIntoVec for Args<P>
where
    P: JsValuesTupleIntoVec,
{
    fn into_vec(self, env: napi::sys::napi_env) -> napi::Result<Vec<napi::sys::napi_value>> {
        Ok([
            vec![unsafe { ToNapiValue::to_napi_value(env, self.0)? }],
            JsValuesTupleIntoVec::into_vec(self.1, env)?,
        ]
        .concat())
    }
}

impl<P: 'static, R> Clone for ThreadsafeFunction<P, R> {
    fn clone(&self) -> Self {
        Self {
            tsfn: self.tsfn.clone(),
            env: self.env,
            _phantom: PhantomData,
        }
    }
}

impl<P: 'static + JsValuesTupleIntoVec, R> FromNapiValue for ThreadsafeFunction<P, R> {
    unsafe fn from_napi_value(env: napi_env, napi_val: napi_value) -> napi::Result<Self> {
        let tsfn = Tsfn::from_napi_value(env, napi_val)?;
        Ok(Self {
            tsfn,
            env,
            _phantom: PhantomData,
        })
    }
}

impl<P: 'static, R: FromNapiValue + Send + 'static> ThreadsafeFunction<P, R> {
    pub fn call(&self, value: P) -> Result<R> {
        let (sender, receiver) = channel();
        let ctx = unsafe { create_js_context(self.env) };
        // load(path)
        // load(ctx, path)
        self.tsfn.call_with_return_value(
            Args(ctx, value),
            ThreadsafeFunctionCallMode::NonBlocking,
            move |r: JsUnknown| {
                if r.is_promise().unwrap() {
                    let promise = Promise::<R>::from_unknown(r).unwrap();
                    spawn(async move {
                        let r = promise.await;
                        sender
                            .send(
                                r.map_err(|e| anyhow!("Tsfn promise rejected {}.", e.to_string())),
                            )
                            .expect("Failed to send napi returned value.");
                    });
                } else {
                    let r = R::from_unknown(r).unwrap();
                    sender
                        .send(Ok(r))
                        .expect("Failed to send napi returned value.");
                }
                Ok(())
            },
        );

        receiver
            .recv()
            .expect("Failed to receive napi returned value.")
    }
}

unsafe impl<T: 'static, R> Sync for ThreadsafeFunction<T, R> {}
unsafe impl<T: 'static, R> Send for ThreadsafeFunction<T, R> {}
