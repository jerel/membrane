use futures::Stream;
use membrane::async_dart;
use membrane::runtime::{App, Interface, JoinHandle};
use std::{fmt::Debug, future::Future};

struct TestRuntime();
impl Interface for TestRuntime {
  fn spawn<T>(&self, future: T) -> JoinHandle
  where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
  {
    JoinHandle {
      debug_id: String::new(),
      abort: Box::new(|| {}),
    }
  }

  fn spawn_blocking<F, R>(&self, future: F) -> JoinHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + Debug + 'static,
  {
    JoinHandle {
      debug_id: String::new(),
      abort: Box::new(|| {}),
    }
  }
}

static RUNTIME: App<TestRuntime> = App::new(|| TestRuntime());

#[async_dart(namespace = "a")]
pub fn one_failure() -> impl Stream<i32, String> {}

#[async_dart(namespace = "a")]
pub fn two_failure() -> impl Stream<Item = i32, String> {}

#[async_dart(namespace = "a")]
pub fn one_success() -> impl Stream<Item = Result<i32, String>> {
  futures::stream::iter(vec![])
}

fn main() {}
