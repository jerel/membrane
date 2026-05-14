use std::os::raw::c_char;

#[doc(hidden)]
#[repr(u8)]
#[derive(serde::Serialize)]
pub enum MembraneResponseKind {
  Data,
  Panic,
}

#[doc(hidden)]
#[repr(C)]
pub struct MembraneResponse {
  pub kind: MembraneResponseKind,
  pub data: *const std::ffi::c_void,
}

#[doc(hidden)]
#[repr(u8)]
#[derive(serde::Serialize)]
pub enum MembraneMsgKind {
  Ok,
  Error,
}

#[doc(hidden)]
pub struct TaskHandle(pub Box<dyn Fn()>);

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_cancel_membrane_task(task_handle: *mut TaskHandle) -> i32 {
  // turn the pointer back into a box and Rust will drop it when it goes out of scope
  let handle = Box::from_raw(task_handle);
  (handle.0)();

  1
}

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_free_membrane_vec(len: i64, ptr: *const u8) -> i32 {
  // turn the pointer back into a vec and Rust will drop it
  #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
  let _ = ::std::slice::from_raw_parts::<u8>(ptr, len as usize);

  1
}

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_free_membrane_string(ptr: *mut c_char) -> i32 {
  // turn the pointer back into a CString and Rust will drop it
  let _ = ::std::ffi::CString::from_raw(ptr);

  1
}

#[doc(hidden)]
#[macro_export]
macro_rules! error {
  ($result:expr) => {
    error!($result, ::std::ptr::null());
  };
  ($result:expr, $error:expr) => {
    match $result {
      Ok(value) => value,
      Err(e) => {
        ::membrane::ffi_helpers::update_last_error(e);
        // silence unreachable code warnings to enable panicking on invalid data
        #[allow(unreachable_code)]
        return $error;
      }
    }
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! cstr {
  ($ptr:expr) => {
    cstr!($ptr, ::std::ptr::null())
  };
  ($ptr:expr, $error:expr) => {{
    ::membrane::ffi_helpers::null_pointer_check!($ptr);
    error!(unsafe { CStr::from_ptr($ptr).to_str() }, $error)
  }};
}
