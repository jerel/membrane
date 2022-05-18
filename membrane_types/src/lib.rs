pub use heck;
pub use proc_macro2;
pub use quote;
pub use syn;

use std::fmt;

pub mod c;
pub mod dart;
pub mod rust;
mod utils;

#[derive(Debug)]
pub struct Input {
  pub variable: String,
  pub rust_type: String,
  pub ty: syn::Type,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutputStyle {
  EmitterSerialized,
  EmitterStreamSerialized,
  StreamSerialized,
  Serialized,
}

impl fmt::Display for OutputStyle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
