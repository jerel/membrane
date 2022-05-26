pub use emitter_impl::{Emitter, Emitter as StreamEmitter, MembraneHandle};
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

  pub type Context = *mut ::std::os::raw::c_void;
  pub type Data = *mut ::std::os::raw::c_void;
  #[repr(C)]
  #[derive(Debug, Copy, Clone)]
  pub struct CHandle {
    pub context: Context,
    pub push: unsafe extern "C" fn(Context, Data),
    drop: unsafe extern "C" fn(Context),
  }

  pub type MembraneHandle = *mut CHandle;

  //
  // Shared emitter implementation
  //
  pub trait Emitter<T>: Send + 'static {
    #[doc(hidden)]
    fn new(_: i64) -> Self;
    #[doc(hidden)]
    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static>;
    fn source<F, D>(&self, _: F) -> CHandle
    where
      F: FnMut(&D) + 'static;
    fn push(&self, _: T) -> Result<(), Ended>;
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

    fn source<F, D>(&self, callback: F) -> CHandle
    where
      F: FnMut(&D) + 'static,
    {
      let ptr = Box::into_raw(Box::new(callback));
      CHandle {
        context: ptr as *mut _,
        push: run_push_closure::<F, D>,
        drop: drop_box::<F>,
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

    fn push(&self, result: Result<T, E>) -> Result<(), Ended> {
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

    fn source<F, D>(&self, callback: F) -> CHandle
    where
      F: FnMut(&D) + 'static,
    {
      self.inner.source(callback)
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      self.inner.abort_handle()
    }

    fn push(&self, result: Result<T, E>) -> Result<(), Ended> {
      let state = self.inner.push(result);
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

    fn source<F, D>(&self, callback: F) -> CHandle
    where
      F: FnMut(&D) + 'static,
    {
      self.inner.source(callback)
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      self.inner.abort_handle()
    }

    fn push(&self, result: Result<T, E>) -> Result<(), Ended> {
      self.inner.push(result)
    }

    fn is_done(&self) -> bool {
      self.inner.is_done()
    }

    fn on_done(&self, func: Box<dyn Fn() + Send + 'static>) {
      self.inner.on_done(func)
    }
  }

  pub extern "C" fn run_push_closure<'r, F, D>(
    closure: *mut std::ffi::c_void,
    data: *mut std::ffi::c_void,
  ) where
    F: FnMut(&'r D),
    D: 'r,
  {
    let ptr = closure as *mut F;
    if ptr.is_null() {
      return eprintln!("run_push_closure was called with a NULL pointer");
    }
    let callback = unsafe { &mut *ptr };

    let data_ptr = data as *mut D;
    if data_ptr.is_null() {
      return eprintln!("run_push_closure was called with a NULL pointer");
    }

    let data = unsafe { &*data_ptr };

    callback(data);
  }

  extern "C" fn drop_box<T>(data: *mut std::ffi::c_void) {
    unsafe {
      if data.is_null() {
        return eprintln!("membrane drop_box was called with a NULL pointer");
      }

      Box::from_raw(data as *mut T);
      println!("cleaning up closure box");
    }
  }

  #[no_mangle]
  pub extern "C" fn membrane_drop_handle(data: *mut std::ffi::c_void) {
    unsafe {
      if data.is_null() {
        return eprintln!("membrane_drop_handle was called with a NULL pointer");
      }

      let handle = Box::from_raw(data as MembraneHandle);
      (handle.drop)(handle.context);
      // `handle` will now be dropped as it goes out of scope
      println!("cleaning up handle box");
    }
  }
}
