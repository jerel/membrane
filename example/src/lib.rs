mod application;
mod data;

use membrane::runtime::{AbortHandle, App, Interface};

pub struct Runtime(tokio::runtime::Runtime);

impl Interface for Runtime {
  fn spawn<F>(&self, future: F) -> AbortHandle
  where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
  {
    let join_handle = self.0.spawn(future);
    AbortHandle::new(move || join_handle.abort())
  }

  fn spawn_blocking<F, R>(&self, future: F) -> AbortHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
  {
    let join_handle = self.0.spawn_blocking(future);
    AbortHandle::new(move || join_handle.abort())
  }
}

static RUNTIME: App<Runtime> = App::new(|| {
  Runtime(
    tokio::runtime::Builder::new_multi_thread()
      .worker_threads(2)
      .thread_name("example")
      .enable_time()
      .enable_io()
      .build()
      .unwrap(),
  )
});

// this is necessary for Rust prior to 1.60 for generator.rs to be able to inspect lib.rs...
// it prevents our "unused" code from being stripped out
pub fn load() {
  #[cfg(feature = "c-example")]
  application::c_threading::load();

  #[cfg(feature = "c-example")]
  application::c_render::load();
}

// Prevent extern "C" functions from being opitmized out by the linker
#[used]
static CANCEL_CALLBACK: unsafe extern "C" fn(*mut membrane::TaskHandle) -> i32 =
  membrane::membrane_cancel_membrane_task;
#[used]
static FREE_VEC: unsafe extern "C" fn(i64, *const u8) -> i32 = membrane::membrane_free_membrane_vec;
