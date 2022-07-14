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

// attribute errors

#[async_dart]
pub async fn no_namespace() -> i32 {}

#[async_dart(namespace = "a", foo = true)]
pub async fn bad_option() -> i32 {}

#[sync_dart(namespace = "a", os_thread = true)]
pub async fn os_thread_option_used_in_sync_fn() -> i32 {}

// return value errors

#[async_dart(namespace = "a")]
pub async fn no_result() -> i32 {}

#[async_dart(namespace = "a")]
pub async fn bare_tuple() -> Result<(i32, i32), String> {}

#[async_dart(namespace = "a")]
pub async fn top_level_option() -> Option<String> {}

#[async_dart(namespace = "a")]
pub async fn return_fn() -> Result<dyn Fn(), String> {}

#[async_dart(namespace = "a")]
pub async fn option_success() -> Result<Option<i32>, String> {
  Ok(Some(1))
}

#[sync_dart(namespace = "a")]
pub fn emitter_in_sync_return() -> impl membrane::emitter::Emitter<Result<String, String>> {
  membrane::emitter::emitter!()
}

// argument errors

#[async_dart(namespace = "a")]
pub async fn failing_arg(self) -> Result<(), String> {
  Ok(())
}

#[async_dart(namespace = "a")]
pub async fn bad_arg_type(one: i32) -> Result<i32, String> {}

#[async_dart(namespace = "a")]
pub async fn failing_arg_two(foo: &[i8]) -> Result<(), String> {
  Ok(())
}

#[async_dart(namespace = "a")]
pub async fn one_success() -> Result<Vec<i32>, String> {
  Ok(vec![10])
}

fn main() {}
