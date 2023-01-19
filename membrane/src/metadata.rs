use crate::{DeferredEnumTrace, DeferredTrace};

pub fn enums() -> Vec<&'static DeferredEnumTrace> {
  inventory::iter::<DeferredEnumTrace>().collect()
}

pub fn functions() -> Vec<&'static DeferredTrace> {
  inventory::iter::<DeferredTrace>().collect()
}

pub fn version() -> &'static str {
  const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
  VERSION.unwrap_or("unknown")
}

pub(crate) fn extract_metadata_from_cdylib(
  lib_path: &String,
  input_libs: &mut Vec<libloading::Library>,
) -> Result<
  (
    Vec<&'static DeferredEnumTrace>,
    Vec<&'static DeferredTrace>,
    Option<&'static str>,
    &'static str,
    &'static str,
  ),
  String,
> {
  match extract_metadata(lib_path, input_libs) {Ok(symbols) => Ok(symbols),
    Err(libloading::Error::DlOpen { desc }) => {
      Err(format!(
        "No dynamic library file found for Membrane: {:?}",
        desc
      ))
    }
    Err(libloading::Error::DlSym { .. }) => {
      Err(format!(
        "Invalid dynamic library file found for Membrane.
        This is likely due to one of the following, in order of likelyhood:
          (1) The cdylib that was passed was compiled in `release` mode instead of `dev` so Membrane metadata was compiled out.
          (2) An arbitrary cdylib file was passed instead of one which was built using Membrane's async_dart/sync_dart macros.
          (3) The crate which compiled the cdylib imports crates which correctly use Membrane macros but it does not itself use any
        of the macros. To solve this case invoke the membrane::export_metadata!() in the cdylib crate's lib.rs file.
          (4) Some other error or bug, possibly an old cdylib being inspected by a new version of Membrane.",
      ))
    }
    Err(err) => {
      Err(format!("Error while loading dynamic library file for Membrane: {:?}", err))
    }
  }
}

fn extract_metadata(
  lib_path: &String,
  input_libs: &mut Vec<libloading::Library>,
) -> Result<
  (
    Vec<&'static DeferredEnumTrace>,
    Vec<&'static DeferredTrace>,
    Option<&'static str>,
    &'static str,
    &'static str,
  ),
  libloading::Error,
> {
  unsafe {
    let lib = libloading::Library::new(lib_path)?;

    let enums: libloading::Symbol<fn() -> Box<Vec<&'static DeferredEnumTrace>>> =
      lib.get(b"membrane_metadata_enums")?;
    let functions: libloading::Symbol<fn() -> Box<Vec<&'static DeferredTrace>>> =
      lib.get(b"membrane_metadata_functions")?;
    let version: libloading::Symbol<fn() -> Option<&'static str>> =
      lib.get(b"membrane_metadata_version")?;
    let git_version: libloading::Symbol<fn() -> &'static str> =
      lib.get(b"membrane_metadata_git_version")?;
    let membrane_version: libloading::Symbol<fn() -> &'static str> =
      lib.get(b"membrane_metadata_membrane_version")?;

    let output = (
      (*(enums)()),
      (*(functions)()),
      (version)(),
      (git_version)(),
      (membrane_version)(),
    );

    // keep a copy of the .so so that it doesn't get unloaded while we're accessing the DeferredTrace values
    input_libs.push(lib);

    Ok(output)
  }
}
