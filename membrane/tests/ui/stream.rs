#[async_dart(namespace = "a")]
pub fn one_failure() -> impl Stream<i32, String> {}

#[async_dart(namespace = "a")]
pub fn two_failure() -> impl Stream<Item = i32, String> {}

#[async_dart(namespace = "a")]
pub fn one_success() -> impl Stream<Item = Result<i32, String>> {
  futures::stream::iter(vec![])
}

use futures::Stream;
use membrane::async_dart;
use membrane::runtime::{App, Interface, AbortHandle};
use std::future::Future;

struct TestRuntime();
impl Interface for TestRuntime {
  fn spawn<T>(&self, future: T) -> AbortHandle
  where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
  {
    AbortHandle::new(|| {})
  }

  fn spawn_blocking<F, R>(&self, future: F) -> AbortHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
  {
    AbortHandle::new(|| {})
  }
}

static RUNTIME: App<TestRuntime> = App::new(|| TestRuntime());

fn main() {}
