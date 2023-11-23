use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use mako_core::rayon::{ThreadPool, ThreadPoolBuilder};

pub fn create_thread_pool<T>() -> (Arc<ThreadPool>, Sender<T>, Receiver<T>) {
    let pool = Arc::new(ThreadPoolBuilder::new().build().unwrap());
    let (rs, rr) = channel();
    (pool, rs, rr)
}
