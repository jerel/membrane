use crate::quote::quote;
use once_cell::sync::OnceCell;
use proc_macro::TokenStream;
use std::env;
use std::path::PathBuf;
use toml::Value;

static BOOTSTRAPPED: OnceCell<bool> = OnceCell::new();

//
// Fetch the crate type from the Cargo.toml of the currently-compiling crate
//
pub(crate) fn is_cdylib() -> bool {
  let toml = std::fs::read_to_string(
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml"),
  )
  .unwrap();

  match toml::from_str::<Value>(&toml) {
    Ok(Value::Table(table)) => match table.get("lib") {
      Some(Value::Table(lib)) => match lib.get("crate-type") {
        Some(Value::Array(crate_type)) => crate_type.iter().any(|x| match x {
          Value::String(val) => val == "cdylib",
          _ => false,
        }),
        _ => false,
      },
      _ => false,
    },
    _ => false,
  }
}

pub(crate) fn maybe_inject_metadata(mut token_stream: TokenStream) -> TokenStream {
  if BOOTSTRAPPED.get().is_none() {
    BOOTSTRAPPED.set(true).unwrap();

    // we only add the metadata once and only then when we're a crate that produces a dylib, otherwise
    // we run the risk of generating duplicate functions within shared workspace crates that all use this macro
    if is_cdylib() {
      token_stream.extend::<TokenStream>(
          quote! {
            #[no_mangle]
            pub fn membrane_metadata_enums() -> Box<Vec<&'static ::membrane::DeferredEnumTrace>> {
              Box::new(::membrane::metadata::enums())
            }

            #[no_mangle]
            pub fn membrane_metadata_functions() -> Box<Vec<&'static ::membrane::DeferredTrace>> {
              Box::new(::membrane::metadata::functions())
            }

            #[no_mangle]
            pub extern "C" fn membrane_metadata_version() -> *mut std::os::raw::c_char {
              // allow the developer to override the embedded version string with one of their own choosing
              const LIB_VERSION: Option<&str> = option_env!("MEMBRANE_CDYLIB_VERSION");

              if let Some(cdylib_version) = LIB_VERSION {
                let version = ::std::ffi::CString::new(cdylib_version).expect("Invalid version string received");
                version.into_raw()
              } else {
              const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
              const GIT_VERSION: &str = ::membrane::git_version!(args = ["--always"], fallback = "unknown");
              let cargo_version = VERSION.unwrap_or_else(|| "unknown");
              let version = ::std::ffi::CString::new(String::new() + cargo_version + "-" + GIT_VERSION).expect("Invalid version string received");
              version.into_raw()
              }
            }

            #[no_mangle]
            pub extern "C" fn membrane_metadata_membrane_version() -> *mut std::os::raw::c_char {
              let version = ::std::ffi::CString::new(::membrane::metadata::version()).expect("Invalid string received");
              version.into_raw()
            }
          }
          .into(),
        );
    }
  }

  token_stream
}
