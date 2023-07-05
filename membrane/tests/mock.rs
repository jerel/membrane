use membrane::runtime::{AbortHandle, App, Interface};
use std::future::Future;

pub struct TestRuntime();

impl Interface for TestRuntime {
  fn spawn<T>(&self, _future: T) -> AbortHandle
  where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
  {
    AbortHandle::new(|| {})
  }

  fn spawn_blocking<F, R>(&self, _future: F) -> AbortHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
  {
    AbortHandle::new(|| {})
  }
}

pub static RUNTIME: App<TestRuntime> = App::new(TestRuntime);
