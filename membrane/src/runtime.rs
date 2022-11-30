use once_cell::sync::OnceCell;
use std::{fmt::Debug, future::Future};

pub trait Interface {
  fn spawn<F>(&self, future: F) -> JoinHandle
  where
    F: Future + Send + 'static,
    F::Output: Send + Debug + 'static;

  fn spawn_blocking<F, R>(&self, future: F) -> JoinHandle
  where
    F: FnOnce() -> R + Send + 'static,
    R: Send + Debug + 'static;
}

pub struct JoinHandle {
  pub debug_id: String,
  pub abort: Box<dyn Fn()>,
}

impl JoinHandle {
  pub fn abort(&self) {
    (self.abort)();
  }
}

impl Debug for JoinHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("JoinHandle")
      .field("debug_id", &self.debug_id)
      .finish()
  }
}

#[derive(Debug)]
pub struct App<T: Interface> {
  builder: fn() -> T,
  runtime: OnceCell<T>,
  metadata: Vec<i32>,
}

impl<T: Interface> App<T> {
  pub const fn new(builder: fn() -> T) -> App<T> {
    App {
      builder,
      runtime: OnceCell::new(),
      metadata: vec![],
    }
  }

  pub fn get(&self) -> &T {
    self.runtime.get_or_init(self.builder)
  }

  pub fn metadata(&self) -> &Vec<i32> {
    &self.metadata
  }
}
