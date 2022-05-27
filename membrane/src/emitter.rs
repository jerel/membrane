pub use emitter_impl::{CHandle, Emitter, Emitter as StreamEmitter};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

pub use membrane_macro::emitter;

type FinalizerCallback = Arc<Mutex<Option<Box<dyn Fn() + Send + 'static>>>>;

#[doc(hidden)]
#[derive(Clone)]
pub struct Handle<T, E> {
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
  use super::PhantomData;
  use super::{Arc, Mutex};
  use crate::emitter::{EmitterData, Handle};
  use serde::Serialize;

  pub type Context = *mut ::std::os::raw::c_void;
  pub type Data = *mut ::std::os::raw::c_void;
  #[repr(C)]
  #[derive(Debug, Copy, Clone)]
  pub struct CHandleImpl {
    pub push_ctx: Context,
    pub push: unsafe extern "C" fn(Context, Data),
    drop_push_ctx: unsafe extern "C" fn(Context),
  }

  pub type CHandle = *mut CHandleImpl;

  //
  // Shared emitter implementation
  //
  pub trait Emitter<T>: Send + 'static {
    fn new(port: i64) -> Self;
    #[doc(hidden)]
    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static>;
    fn on_data<F, D>(&self, _: F) -> *mut CHandleImpl
    where
      F: FnMut(&D) + 'static;
    fn push(&self, _: T) -> bool;
    fn is_done(&self) -> bool;
    fn on_done(&self, _: Box<dyn Fn() + Send + 'static>);
  }

  #[allow(clippy::mutex_atomic)]
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for EmitterData<T, E>
  {
    fn new(port: i64) -> Self {
      EmitterData::<T, E> {
        _type: PhantomData,
        is_done: Arc::new(Mutex::new(false)),
        isolate: allo_isolate::Isolate::new(port),
        on_done_callback: Arc::new(Mutex::new(None)),
      }
    }

    fn on_data<F, D>(&self, callback: F) -> *mut CHandleImpl
    where
      F: FnMut(&D) + 'static,
    {
      let ptr = Box::into_raw(Box::new(callback));
      Box::into_raw(Box::new(CHandleImpl {
        push_ctx: ptr as *mut _,
        push: run_push_closure::<F, D>,
        drop_push_ctx: drop_push_ctx::<F>,
      }))
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

    fn push(&self, result: Result<T, E>) -> bool {
      crate::utils::send::<T, E>(self.isolate, result)
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

  #[allow(clippy::mutex_atomic)]
  impl<T: Serialize + Send + 'static, E: Serialize + Send + 'static> Emitter<Result<T, E>>
    for Handle<T, E>
  {
    fn new(port: i64) -> Self {
      Handle::<T, E> {
        inner: EmitterData::<T, E> {
          _type: PhantomData,
          is_done: Arc::new(Mutex::new(false)),
          isolate: allo_isolate::Isolate::new(port),
          on_done_callback: Arc::new(Mutex::new(None)),
        },
      }
    }

    fn on_data<F, D>(&self, callback: F) -> *mut CHandleImpl
    where
      F: FnMut(&D) + 'static,
    {
      self.inner.on_data(callback)
    }

    fn abort_handle(&self) -> Box<dyn Fn() + Send + 'static> {
      self.inner.abort_handle()
    }

    fn push(&self, result: Result<T, E>) -> bool {
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

  extern "C" fn drop_push_ctx<T>(data: *mut std::ffi::c_void) {
    unsafe {
      if data.is_null() {
        return eprintln!("membrane drop_push_ctx was called with a NULL pointer");
      }

      Box::from_raw(data as *mut T);
    }
  }

  #[no_mangle]
  pub extern "C" fn membrane_drop_handle(data: *mut std::ffi::c_void) {
    unsafe {
      if data.is_null() {
        return eprintln!("membrane_drop_handle was called with a NULL pointer");
      }

      let handle = Box::from_raw(data as CHandle);
      (handle.drop_push_ctx)(handle.push_ctx);
      // `handle` will now be dropped as it goes out of scope
    }
  }
}
