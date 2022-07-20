use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

mod application;
mod data;

pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
  Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("example")
    .enable_time()
    .enable_io()
    .build()
    .unwrap()
});

// this is necessary for Rust prior to 1.60 for generator.rs to be able to inspect lib.rs...
// it prevents our "unused" code from being stripped out
pub fn load() {
  #[cfg(feature = "c-example")]
  application::c_example::load();

  #[cfg(feature = "c-example")]
  application::c_render::load();
}
