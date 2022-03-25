#![forbid(unsafe_code)]

use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

mod application;
mod data;

pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
  Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("example")
    .enable_time()
    .build()
    .unwrap()
});

// this is necessary for bin.rs to be able to inspect lib.rs
pub fn load() {}
