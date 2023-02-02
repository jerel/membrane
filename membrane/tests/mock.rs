use membrane::runtime::{App, Interface, JoinHandle};
use std::future::Future;

pub struct TestRuntime();

impl Interface for TestRuntime {
  fn spawn<T>(&self, _future: T) -> JoinHandle
  where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
  {
    JoinHandle {
      abort: Box::new(|| {}),
    }
  }

  fn spawn_blocking<F, R>(&self, _future: F) -> JoinHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
  {
    JoinHandle {
      abort: Box::new(|| {}),
    }
  }
}

pub static RUNTIME: App<TestRuntime> = App::new(TestRuntime);
