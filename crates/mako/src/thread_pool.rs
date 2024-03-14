use mako_core::lazy_static::lazy_static;
use mako_core::rayon::{ThreadPool, ThreadPoolBuilder};

lazy_static! {
    static ref THREAD_POOL: ThreadPool = ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon thread {}", i))
        .build()
        .expect("failed to create rayon thread pool.");
}

pub fn spawn<F>(func: F)
where
    F: FnOnce() + Send + 'static,
{
    THREAD_POOL.spawn(func)
}
