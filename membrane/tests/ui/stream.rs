use futures::{Future, Stream};
use membrane::async_dart;

struct JoinHandle {}
impl JoinHandle {
  pub fn abort(&self) {}
}

struct Runtime {}
impl Runtime {
  pub fn spawn<T>(&self, future: T) -> JoinHandle
  where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
  {
    JoinHandle {}
  }
}

static RUNTIME: Runtime = Runtime {};

#[async_dart(namespace = "a")]
pub fn one_failure() -> impl Stream<i32, String> {}

#[async_dart(namespace = "a")]
pub fn two_failure() -> impl Stream<Item = i32, String> {}

#[async_dart(namespace = "a")]
pub fn three_failure() -> impl Stream<Item = Result<Option<i32>, String>> {}

#[async_dart(namespace = "a")]
pub fn one_success() -> impl Stream<Item = Result<i32, String>> {
  futures::stream::iter(vec![])
}

fn main() {}
