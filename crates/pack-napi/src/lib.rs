/*
Copyright (c) 2017 The swc Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
*/

#![recursion_limit = "2048"]
//#![deny(clippy::all)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

use std::{cell::OnceCell, sync::Once};

use tokio::runtime::Runtime;

#[macro_use]
extern crate napi_derive;

pub mod pack_api;
pub mod util;

#[cfg(not(any(feature = "__internal_dhat-heap", feature = "__internal_dhat-ad-hoc")))]
#[global_allocator]
static ALLOC: turbo_tasks_malloc::TurboMalloc = turbo_tasks_malloc::TurboMalloc;

#[cfg(feature = "__internal_dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(not(target_arch = "wasm32"))]
#[napi::module_init]

fn init() {
    use tokio::runtime::Builder;
    use turbo_tasks_malloc::TurboMalloc;

    let rt = Builder::new_multi_thread()
        .enable_all()
        .on_thread_stop(|| {
            TurboMalloc::thread_stop();
        })
        .disable_lifo_slot()
        .build()
        .unwrap();
    create_custom_tokio_runtime(rt);
}

static mut USER_DEFINED_RT: OnceCell<Option<Runtime>> = OnceCell::new();

pub fn create_custom_tokio_runtime(rt: Runtime) {
    unsafe {
        #[allow(static_mut_refs)]
        USER_DEFINED_RT.get_or_init(move || Some(rt));
    }
}

static REGISTER_ONCE: Once = Once::new();

#[cfg(not(target_arch = "wasm32"))]
fn register() {
    REGISTER_ONCE.call_once(|| {
        ::pack_api::register();
        pack_core::register();
        include!(concat!(env!("OUT_DIR"), "/register.rs"));
    });
}

#[cfg(target_arch = "wasm32")]
fn register() {
    //noop
}
