use crate::{DeferredEnumTrace, DeferredTrace};

pub fn enums() -> Vec<&'static DeferredEnumTrace> {
  inventory::iter::<DeferredEnumTrace>().collect()
}

pub fn functions() -> Vec<&'static DeferredTrace> {
  inventory::iter::<DeferredTrace>().collect()
}

pub(crate) fn extract_metadata_from_cdylib(
  lib_path: &String,
  input_libs: &mut Vec<libloading::Library>,
) -> (
  Vec<&'static DeferredEnumTrace>,
  Vec<&'static DeferredTrace>,
  Option<&'static str>,
  &'static str,
) {
  unsafe {
    let lib = libloading::Library::new(lib_path).unwrap();

    let enums: libloading::Symbol<fn() -> Box<Vec<&'static DeferredEnumTrace>>> =
      lib.get(b"membrane_metadata_enums").unwrap();
    let functions: libloading::Symbol<fn() -> Box<Vec<&'static DeferredTrace>>> =
      lib.get(b"membrane_metadata_functions").unwrap();
    let version: libloading::Symbol<fn() -> Option<&'static str>> =
      lib.get(b"membrane_metadata_version").unwrap();
    let git_version: libloading::Symbol<fn() -> &'static str> =
      lib.get(b"membrane_metadata_git_version").unwrap();

    let output = ((*(enums)()), (*(functions)()), (version)(), (git_version)());

    // keep a copy of the .so so that it doesn't get unloaded while we're accessing the DeferredTrace values
    input_libs.push(lib);

    output
  }
}
