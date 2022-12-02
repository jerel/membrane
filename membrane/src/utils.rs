use crate::{DeferredEnumTrace, DeferredTrace};
use allo_isolate::Isolate;
use serde::ser::Serialize;

pub fn send<T: Serialize, E: Serialize>(isolate: Isolate, result: Result<T, E>) -> bool {
  match result {
    Ok(value) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneMsgKind::Ok as u8, value)) {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
    Err(err) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneMsgKind::Error as u8, err)) {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
  }
}

pub(crate) fn extract_types_from_cdylib(
  lib_path: &String,
  input_libs: &mut Vec<libloading::Library>,
) -> (Vec<&'static DeferredEnumTrace>, Vec<&'static DeferredTrace>) {
  unsafe {
    let lib = libloading::Library::new(lib_path).unwrap();

    let enums: libloading::Symbol<fn() -> Box<Vec<&'static DeferredEnumTrace>>> =
      lib.get(b"membrane_metadata_get_enums").unwrap();
    let functions: libloading::Symbol<fn() -> Box<Vec<&'static DeferredTrace>>> =
      lib.get(b"membrane_metadata_get_functions").unwrap();

    let output = ((*(enums)()), (*(functions)()));

    // keep a copy of the .so so that it doesn't get unloaded while we're accessing the DeferredTrace values
    input_libs.push(lib);

    output
  }
}
