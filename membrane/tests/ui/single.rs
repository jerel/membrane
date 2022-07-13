use futures::Future;
use membrane::{async_dart, sync_dart};

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
pub async fn no_result() -> i32 {}

#[async_dart(namespace = "a")]
pub async fn bare_tuple() -> Result<(i32, i32), String> {}

#[async_dart(namespace = "a")]
pub async fn option_success() -> Result<Option<i32>, String> {
  Ok(Some(1))
}

#[async_dart(namespace = "a")]
pub async fn one_success() -> Result<Vec<i32>, String> {
  Ok(vec![10])
}

#[sync_dart(namespace = "a")]
pub fn emitter_in_sync_return() -> impl membrane::emitter::Emitter<Result<String, String>> {
  membrane::emitter::emitter!()
}

fn main() {}
