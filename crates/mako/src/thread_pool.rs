use std::sync::OnceLock;

use mako_core::rayon::{self, Scope, ThreadPool, ThreadPoolBuilder};

static THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

#[cfg(not(target_family = "wasm"))]
fn build_rayon_thread_pool() -> ThreadPool {
    ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon thread {}", i))
        .build()
        .expect("failed to create rayon thread pool.")
}

#[cfg(not(target_family = "wasm"))]
pub fn spawn<F>(func: F)
where
    F: FnOnce() + Send + 'static,
{
    THREAD_POOL.get_or_init(build_rayon_thread_pool).spawn(func)
}

#[cfg(not(target_family = "wasm"))]
pub fn scope<'scope, OP, R>(op: OP) -> R
where
    OP: FnOnce(&Scope<'scope>) -> R + Send,
    R: Send,
{
    THREAD_POOL.get_or_init(build_rayon_thread_pool).scope(op)
}

#[cfg(all(target_family = "wasm", target_os = "wasi"))]
pub fn spawn<F>(func: F)
where
    F: FnOnce() + Send + 'static,
{
    func()
}

#[cfg(all(target_family = "wasm", target_os = "wasi"))]
pub fn scope<'scope, OP, R>(op: OP) -> R
where
    OP: FnOnce(&Scope<'scope>) -> R + Send,
    R: Send,
{
    rayon::scope(op)
}
