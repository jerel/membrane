use once_cell::sync::OnceCell;
use std::future::Future;

pub trait Interface {
  fn spawn<F>(&self, future: F) -> JoinHandle
  where
    F: Future + Send + 'static,
    F::Output: Send + 'static;

  fn spawn_blocking<F, R>(&self, future: F) -> JoinHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static;
}

pub struct JoinHandle {
  pub abort: Box<dyn Fn()>,
}

impl JoinHandle {
  pub fn abort(&self) {
    (self.abort)();
  }
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
