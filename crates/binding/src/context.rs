use std::{
    ffi::{c_void, CString},
    ptr,
};

use napi::{
    bindgen_prelude::FromNapiValue,
    sys::{
        napi_callback_info, napi_create_function, napi_create_object, napi_env,
        napi_get_cb_info, napi_value,
    },
    Env, JsFunction, JsObject, JsUnknown, NapiRaw,
};

const WARN: &str = "warn";

/// create a js object context with only warn method
/// # Safety
/// calling [napi_create_object]
pub unsafe fn create_js_context(raw_env: napi_env) -> JsObject {
    let mut js_context_ptr = ptr::null_mut();
    let mut js_context = {
        napi_create_object(raw_env, &mut js_context_ptr);
        JsObject::from_napi_value(raw_env, js_context_ptr).unwrap()
    };

    let len = WARN.len();
    let s = CString::new(WARN).unwrap();
    let mut func = ptr::null_mut();

    napi_create_function(
        raw_env,
        s.as_ptr(),
        len,
        Some(warn),
        std::ptr::null_mut(),
        &mut func,
    );

    js_context
        .set_named_property(WARN, JsFunction::from_napi_value(raw_env, func).unwrap())
        .unwrap();

    js_context
}

unsafe extern "C" fn warn(env: napi_env, info: napi_callback_info) -> napi_value {
    let mut argv = [std::ptr::null_mut()];
    napi_get_cb_info(
        env,
        info,
        &mut 1,
        argv.as_mut_ptr(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );

    let message: String = Env::from_raw(env)
        .from_js_value(JsUnknown::from_napi_value(env, argv[0]).unwrap())
        .expect("Argument 0 should be a string when calling warn");

    println!("Warning: {}", message);

    Env::from_raw(env).get_undefined().unwrap().raw()
}
