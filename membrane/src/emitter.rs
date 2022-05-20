pub use emitter_impl::{Emitter, Emitter as StreamEmitter};
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

type FinalizerCallback = Arc<Mutex<Option<Box<dyn Fn() + Send + 'static>>>>;

#[derive(Debug)]
pub struct Ended;

impl fmt::Display for Ended {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "End of Stream")
  }
}

impl std::error::Error for Ended {}

#[doc(hidden)]
#[derive(Clone)]
pub struct Handle<T, E> {
  inner: EmitterData<T, E>,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct StreamHandle<T, E> {
  inner: EmitterData<T, E>,
}

#[doc(hidden)]
#[derive(Clone)]
struct EmitterData<T, E> {
  // pass the types through so that we know what we're serializing
  _type: PhantomData<Result<T, E>>,
  isolate: allo_isolate::Isolate,
  is_done: Arc<Mutex<bool>>,
  on_done_callback: FinalizerCallback,
}

mod emitter_impl {
  use super::Ended;
  use super::PhantomData;
  use super::{Arc, Mutex};
  use crate::emitter::{EmitterData, Handle, StreamHandle};
  use serde::Serialize;

  //
  // Shared emitter implementation
  //
  pub trait Emitter<T>: Send + 'static {
    #[doc(hidden)]
    fn new(_: i64) -> Self;
    #[doc(hidden)]
    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static>;
    fn call(&self, _: T) -> Result<(), Ended>;
    fn is_done(&self) -> bool;
    fn on_done(&self, _: Box<dyn Fn() + Send + 'static>);
  }

  #[allow(clippy::mutex_atomic)]
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for EmitterData<T, E>
  {
    fn new(port: i64) -> Self {
      let isolate = allo_isolate::Isolate::new(port);
      EmitterData::<T, E> {
        _type: PhantomData,
        is_done: Arc::new(Mutex::new(false)),
        isolate,
        on_done_callback: Arc::new(Mutex::new(None)),
      }
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      let is_done = self.is_done.clone();
      let finalizer = self.on_done_callback.clone();
      Box::new(move || {
        let mut done = is_done.lock().unwrap();
        *done = true;
        let func = finalizer.lock().unwrap();
        if let Some(func) = &*func {
          (func)();
        }
      })
    }

    fn call(&self, result: Result<T, E>) -> Result<(), Ended> {
      if self.is_done() {
        return Err(Ended);
      }
      crate::utils::send::<T, E>(self.isolate, result);
      Ok(())
    }

    fn is_done(&self) -> bool {
      let is_done = self.is_done.lock().unwrap();
      *is_done
    }

    fn on_done(&self, func: Box<dyn Fn() + Send + 'static>) {
      let mut on_done = self.on_done_callback.lock().unwrap();
      *on_done = Some(func);
    }
  }

  //
  // Oneshot implementation
  ///
  #[allow(clippy::mutex_atomic)]
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for Handle<T, E>
  {
    fn new(port: i64) -> Self {
      let isolate = allo_isolate::Isolate::new(port);
      Handle::<T, E> {
        inner: EmitterData::<T, E> {
          _type: PhantomData,
          is_done: Arc::new(Mutex::new(false)),
          isolate,
          on_done_callback: Arc::new(Mutex::new(None)),
        },
      }
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      self.inner.abort_handle()
    }

    fn call(&self, result: Result<T, E>) -> Result<(), Ended> {
      let state = self.inner.call(result);
      // we preemptively show this emitter as done without waiting for the finalizer
      // callback to do it since it should not be called more than once anyway
      if state.is_ok() {
        let mut done = self.inner.is_done.lock().unwrap();
        *done = true;
      }

      state
    }

    fn is_done(&self) -> bool {
      self.inner.is_done()
    }

    fn on_done(&self, func: Box<dyn Fn() + Send + 'static>) {
      self.inner.on_done(func)
    }
  }

  //
  // Stream implementation
  ///
  #[allow(clippy::mutex_atomic)]
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for StreamHandle<T, E>
  {
    fn new(port: i64) -> Self {
      let isolate = allo_isolate::Isolate::new(port);
      StreamHandle::<T, E> {
        inner: EmitterData::<T, E> {
          _type: PhantomData,
          is_done: Arc::new(Mutex::new(false)),
          isolate,
          on_done_callback: Arc::new(Mutex::new(None)),
        },
      }
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      self.inner.abort_handle()
    }

    fn call(&self, result: Result<T, E>) -> Result<(), Ended> {
      self.inner.call(result)
    }

    fn is_done(&self) -> bool {
      self.inner.is_done()
    }

    fn on_done(&self, func: Box<dyn Fn() + Send + 'static>) {
      self.inner.on_done(func)
    }
  }
}
