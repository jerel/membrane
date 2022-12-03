use crate::{DeferredEnumTrace, DeferredTrace};

pub fn enums() -> Vec<&'static DeferredEnumTrace> {
  inventory::iter::<DeferredEnumTrace>().collect()
}

pub fn functions() -> Vec<&'static DeferredTrace> {
  inventory::iter::<DeferredTrace>().collect()
}

pub fn version() -> &'static str {
  const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
  VERSION.unwrap_or_else(|| "unknown")
}

pub(crate) fn extract_metadata_from_cdylib(
  lib_path: &String,
  input_libs: &mut Vec<libloading::Library>,
) -> (
  Vec<&'static DeferredEnumTrace>,
  Vec<&'static DeferredTrace>,
  Option<&'static str>,
  &'static str,
  &'static str,
) {
  unsafe {
    let lib = libloading::Library::new(lib_path).unwrap();

    let enums: libloading::Symbol<fn() -> Box<Vec<&'static DeferredEnumTrace>>> = lib
      .get(b"membrane_metadata_enums")
      .expect("Invalid .so file found for Membrane");
    let functions: libloading::Symbol<fn() -> Box<Vec<&'static DeferredTrace>>> = lib
      .get(b"membrane_metadata_functions")
      .expect("Invalid .so file found for Membrane");
    let version: libloading::Symbol<fn() -> Option<&'static str>> = lib
      .get(b"membrane_metadata_version")
      .expect("Invalid .so file found for Membrane");
    let git_version: libloading::Symbol<fn() -> &'static str> = lib
      .get(b"membrane_metadata_git_version")
      .expect("Invalid .so file found for Membrane");
    let membrane_version: libloading::Symbol<fn() -> &'static str> = lib
      .get(b"membrane_metadata_membrane_version")
      .expect("Invalid .so file found for Membrane");

    let output = (
      (*(enums)()),
      (*(functions)()),
      (version)(),
      (git_version)(),
      (membrane_version)(),
    );

    // keep a copy of the .so so that it doesn't get unloaded while we're accessing the DeferredTrace values
    input_libs.push(lib);

    output
  }
}
