use std::sync::OnceLock;

use mako_core::rayon::{ThreadPool, ThreadPoolBuilder};

static THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

fn build_rayon_thread_pool() -> ThreadPool {
    ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon thread {}", i))
        .build()
        .expect("failed to create rayon thread pool.")
}

pub fn spawn<F>(func: F)
where
    F: FnOnce() + Send + 'static,
{
    THREAD_POOL.get_or_init(build_rayon_thread_pool).spawn(func)
}

pub fn install<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    THREAD_POOL.get_or_init(build_rayon_thread_pool).install(op)
}
