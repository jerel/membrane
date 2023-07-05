use once_cell::sync::OnceCell;
use std::future::Future;

pub trait Interface {
  fn spawn<F>(&self, future: F) -> AbortHandle
  where
    F: Future + Send + 'static,
    F::Output: Send + 'static;

  fn info_spawn<F>(&self, future: F, _info: Info) -> AbortHandle
  where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
  {
    self.spawn(future)
  }

  fn spawn_blocking<F, R>(&self, future: F) -> AbortHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static;

  fn info_spawn_blocking<F, R>(&self, future: F, _info: Info) -> AbortHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
  {
    self.spawn_blocking(future)
  }
}

#[deprecated(
  since = "0.9.5",
  note = "please use `AbortHandle` instead, renamed to better match common tokio naming convention"
)]
pub type JoinHandle = AbortHandle;

pub struct AbortHandle {
  #[deprecated(since = "0.9.5", note = "please use `AbortHandle::new()` instead")]
  pub abort: Box<dyn Fn() + Send + 'static>,
}

impl AbortHandle {
  pub fn new(abort: impl Fn() + Send + 'static) -> Self {
    #[allow(deprecated)]
    Self {
      abort: Box::new(abort),
    }
  }

  pub fn abort(&self) {
    #[allow(deprecated)]
    (self.abort)();
  }
}

unsafe impl Send for AbortHandle {}
unsafe impl Sync for AbortHandle {}

pub struct Info<'a> {
  pub name: &'a str,
}

#[derive(Debug)]
pub struct App<T: Interface> {
  builder: fn() -> T,
  runtime: OnceCell<T>,
}

impl<T: Interface> App<T> {
  pub const fn new(builder: fn() -> T) -> App<T> {
    App {
      builder,
      runtime: OnceCell::new(),
    }
  }

  pub fn get(&self) -> &T {
    self.runtime.get_or_init(self.builder)
  }
}
