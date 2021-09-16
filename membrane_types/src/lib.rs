pub use proc_macro2;
pub use quote;
pub use syn;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote as q;
use std::fmt;
use syn::Ident;

#[derive(Debug)]
pub struct Input {
  pub variable: String,
  pub rust_type: String,
  pub ty: syn::Type,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutputStyle {
  StreamSerialized,
  Serialized,
}

impl fmt::Display for OutputStyle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

pub struct RustExternParams(Vec<TokenStream2>);
pub struct RustTransform(Vec<TokenStream2>);
pub struct RustArgs(Vec<Ident>);

impl From<&Vec<Input>> for RustExternParams {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      let variable = Ident::new(&input.variable, Span::call_site());
      let c_type = rust_c_type(&input.rust_type);
      stream.push(q!(#variable: #c_type))
    }

    Self(stream)
  }
}

impl From<&Vec<Input>> for RustTransform {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      let variable = Ident::new(&input.variable, Span::call_site());
      let cast = cast_c_type_to_rust(&input.rust_type, &input.variable);
      stream.push(q!(let #variable = #cast;))
    }

    Self(stream)
  }
}

impl From<&Vec<Input>> for RustArgs {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(Ident::new(&input.variable, Span::call_site()))
    }

    Self(stream)
  }
}

impl From<RustExternParams> for Vec<TokenStream2> {
  fn from(types: RustExternParams) -> Self {
    types.0
  }
}

impl From<RustTransform> for Vec<TokenStream2> {
  fn from(types: RustTransform) -> Self {
    types.0
  }
}

impl From<RustArgs> for Vec<Ident> {
  fn from(types: RustArgs) -> Self {
    types.0
  }
}

fn rust_c_type(ty: &str) -> TokenStream2 {
  match ty {
    "String" => q!(*const ::std::os::raw::c_char),
    _ => panic!("c type not yet supported"),
  }
}

fn cast_c_type_to_rust(ty: &str, variable: &str) -> TokenStream2 {
  match ty {
    "String" => {
      let variable = Ident::new(variable, Span::call_site());
      q!(cstr!(#variable).to_string())
    }
    _ => panic!("casting c type not yet supported"),
  }
}
