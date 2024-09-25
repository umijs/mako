use std::marker::PhantomData;

use anyhow::Result;
use napi::bindgen_prelude::{spawn, FromNapiValue, JsValuesTupleIntoVec, Promise};
use napi::sys::{napi_env, napi_value};
use napi::threadsafe_function::{
    ErrorStrategy, ThreadsafeFunction as Tsfn, ThreadsafeFunctionCallMode,
};
use napi::JsUnknown;
use oneshot::channel;

pub struct ThreadsafeFunction<P: 'static, R> {
    tsfn: Tsfn<P, ErrorStrategy::Fatal>,
    env: napi_env,
    _phantom: PhantomData<R>,
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
        self.tsfn.call_with_return_value(
            value,
            ThreadsafeFunctionCallMode::NonBlocking,
            move |r: JsUnknown| {
                if r.is_promise().unwrap() {
                    let promise = Promise::<R>::from_unknown(r).unwrap();
                    spawn(async move {
                        let r = promise.await.unwrap();
                        sender.send(r).expect("Failed to send napi returned value.");
                    });
                } else {
                    let r = R::from_unknown(r).unwrap();
                    sender.send(r).expect("Failed to send napi returned value.");
                }
                Ok(())
            },
        );
        let ret = receiver
            .recv()
            .expect("Failed to receive napi returned value.");
        Ok(ret)
    }
}

unsafe impl<T: 'static, R> Sync for ThreadsafeFunction<T, R> {}
unsafe impl<T: 'static, R> Send for ThreadsafeFunction<T, R> {}
