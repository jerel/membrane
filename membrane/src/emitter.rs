pub use emitter_impl::{Emitter, Emitter as StreamEmitter};
use std::cell::Cell;

#[derive(PartialEq, Debug)]
pub enum State {
  Closed,
  Open,
  Sent,
}

pub struct Handle<T, E> {
  _type: Option<Result<T, E>>,
  isolate: allo_isolate::Isolate,
  pub is_done: Cell<bool>,
}

pub struct StreamHandle<T, E> {
  _type: Option<Result<T, E>>,
  isolate: allo_isolate::Isolate,
  pub is_done: Cell<bool>,
}

mod emitter_impl {
  use crate::emitter::Handle;
  use crate::emitter::State;
  use crate::emitter::StreamHandle;
  use serde::Serialize;
  use std::cell::Cell;

  //
  // Oneshot implementation
  //
  pub trait Emitter<T>: Send + 'static {
    fn new(_: i64) -> Self;
    fn call(&self, _: T) -> State;
    #[doc(hidden)]
    fn done(&self);
  }

  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for Handle<T, E>
  {
    fn new(port: i64) -> Self {
      let isolate = allo_isolate::Isolate::new(port);
      Handle::<T, E> {
        _type: None,
        is_done: Cell::from(false),
        isolate,
      }
    }

    fn call(&self, result: Result<T, E>) -> State {
      if self.is_done.get() {
        return State::Closed;
      }
      crate::utils::send::<T, E>(self.isolate, result);
      self.done();
      State::Sent
    }

    fn done(&self) {
      self.is_done.set(true);
    }
  }

  //
  // Stream implementation
  ///
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for StreamHandle<T, E>
  {
    fn new(port: i64) -> Self {
      let isolate = allo_isolate::Isolate::new(port);
      StreamHandle::<T, E> {
        _type: None,
        is_done: Cell::from(false),
        isolate,
      }
    }

    fn call(&self, result: Result<T, E>) -> State {
      if self.is_done.get() {
        return State::Closed;
      }
      crate::utils::send::<T, E>(self.isolate, result);
      State::Open
    }

    fn done(&self) {
      self.is_done.set(true);
    }
  }
}
