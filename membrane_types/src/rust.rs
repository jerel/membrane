use crate::utils::extract_type_from_option;
use crate::Input;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote as q;
use syn::{Ident, Type};

pub struct RustExternParams(Vec<TokenStream2>);
pub struct RustTransforms(Vec<TokenStream2>);
pub struct RustArgs(Vec<Ident>);

impl std::convert::TryFrom<&Vec<Input>> for RustExternParams {
  type Error = syn::Error;

  fn try_from(inputs: &Vec<Input>) -> Result<Self, Self::Error> {
    let mut stream = vec![];

    for input in inputs {
      let variable = Ident::new_raw(&input.variable, Span::call_site());
      let c_type = rust_c_type(
        &flatten_types(&input.ty, vec![])?
          .iter()
          .map(|x| x.as_str())
          .collect::<Vec<&str>>(),
        &input.ty,
      )?;
      stream.push(q!(#variable: #c_type))
    }

    Ok(Self(stream))
  }
}

impl std::convert::TryFrom<&Vec<Input>> for RustTransforms {
  type Error = syn::Error;

  fn try_from(inputs: &Vec<Input>) -> Result<Self, Self::Error> {
    let mut stream = vec![];

    for input in inputs {
      let variable = Ident::new_raw(&input.variable, Span::call_site());
      let cast = cast_c_type_to_rust(
        &flatten_types(&input.ty, vec![])?
          .iter()
          .map(|x| x.as_str())
          .collect::<Vec<&str>>(),
        &input.variable,
        &input.ty,
      )?;
      stream.push(q!(let #variable = #cast;))
    }

    Ok(Self(stream))
  }
}

impl From<&Vec<Input>> for RustArgs {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(Ident::new_raw(&input.variable, Span::call_site()))
    }

    Self(stream)
  }
}

impl From<RustExternParams> for Vec<TokenStream2> {
  fn from(types: RustExternParams) -> Self {
    types.0
  }
}

impl From<RustTransforms> for Vec<TokenStream2> {
  fn from(types: RustTransforms) -> Self {
    types.0
  }
}

impl From<RustArgs> for Vec<Ident> {
  fn from(types: RustArgs) -> Self {
    types.0
  }
}

pub fn flatten_types(ty: &syn::Type, mut types: Vec<String>) -> syn::Result<Vec<String>> {
  match &ty {
    syn::Type::Tuple(_expr) => Ok(vec!["()".to_string()]),
    syn::Type::Path(expr) => {
      let last = expr.path.segments.last().unwrap();
      types.push(last.ident.to_string());

      if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
        args, ..
      }) = &last.arguments
      {
        match args.last() {
          Some(syn::GenericArgument::Type(last)) => flatten_types(last, types),
          _ => Ok(types),
        }
      } else {
        Ok(types)
      }
    }
    _ => Err(syn::Error::new_spanned(
      ty,
      "not a supported argument type for Dart interop",
    )),
  }
}

fn rust_c_type(ty: &[&str], type_: &syn::Type) -> syn::Result<TokenStream2> {
  let result = match ty[..] {
    ["String"] => q!(*const ::std::os::raw::c_char),
    ["i64"] => q!(::std::os::raw::c_longlong),
    ["f64"] => q!(::std::os::raw::c_double),
    ["bool"] => q!(::std::os::raw::c_char), // i8
    ["Vec", ..] => q!(*const u8),
    [serialized] if serialized != "Option" => q!(*const u8),
    ["Option", "String"] => q!(*const ::std::os::raw::c_char),
    ["Option", "i64"] => q!(*const ::std::os::raw::c_longlong),
    ["Option", "f64"] => q!(*const ::std::os::raw::c_double),
    ["Option", "bool"] => q!(*const ::std::os::raw::c_char), // i8
    ["Option", ..] => q!(*const u8),
    _ => {
      return Err(syn::Error::new_spanned(
        type_,
        "not a supported argument type for Dart interop",
      ))
    }
  };

  Ok(result)
}

fn cast_c_type_to_rust(types: &[&str], variable: &str, ty: &Type) -> syn::Result<TokenStream2> {
  let result = match types[..] {
    ["String"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q!(cstr!(#variable, panic!("invalid C string")).to_string())
    }
    ["i64"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q!(#variable)
    }
    ["f64"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q!(#variable)
    }
    ["bool"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q!(#variable != 0)
    }
    // this also handles Vec
    [serialized, ..] if serialized != "Option" => {
      let variable_name = variable;
      let variable = Ident::new_raw(variable, Span::call_site());
      let deserialize = deserialize(variable, variable_name, ty, types[0]);
      q! {
        {
          #deserialize
        }
      }
    }
    ["Option", "String"] => {
      let variable_name = variable;
      let variable = Ident::new_raw(variable, Span::call_site());
      q! {
        match unsafe { #variable.as_ref() } {
          Some(val) => {
            match unsafe { CStr::from_ptr(val).to_str() } {
              Ok(s) => Some(s.to_string()),
              Err(e) => {
                panic!("An invalid string {:?} was received for {}. {:?}", val, #variable_name, e);
              }
            }
          },
          None => None
        }
      }
    }
    ["Option", "i64"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q! {
        match unsafe { #variable.as_ref() } {
          Some(val) => Some(*val),
          None => None
        }
      }
    }
    ["Option", "f64"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q! {
        match unsafe { #variable.as_ref() } {
          Some(val) => Some(*val),
          None => None
        }
      }
    }
    ["Option", "bool"] => {
      let variable = Ident::new_raw(variable, Span::call_site());
      q! {
        match unsafe { #variable.as_ref() } {
          Some(val) => Some(*val != 0),
          None => None
        }
      }
    }
    ["Option", ..] => {
      let variable_name = variable;
      let variable = Ident::new_raw(variable, Span::call_site());
      let ty = extract_type_from_option(ty).unwrap();
      let str_ty = q!(#ty).to_string().split_whitespace().collect::<String>();

      let deserialize = deserialize(variable.clone(), variable_name, ty, str_ty.as_str());

      q! {
        {
          match unsafe { #variable.as_ref() } {
            None => None,
            Some(#variable) => {
              Some({
                #deserialize
              })
            }
          }
        }
      }
    }

    _ => {
      return Err(syn::Error::new_spanned(
        ty,
        "not a supported argument type for Dart interop",
      ))
    }
  };

  Ok(result)
}

fn deserialize(variable: Ident, variable_name: &str, ty: &Type, str_ty: &str) -> TokenStream2 {
  q! {
    let data = unsafe {
      use std::convert::TryInto;
      // read the first 8 bytes to get the length of the full payload (which includes the length byte)
      let bytes = ::std::slice::from_raw_parts::<u8>(#variable, 8);
      // deserialize the bytes to an unsigned integer
      let length = ::membrane::bincode::deserialize::<u64>(&bytes).expect(
        format!("Unable to read the payload length for variable '{}' of type '{}'", #variable_name, #str_ty).as_str()
      );
      let elements: usize = length.try_into().expect("Unable to fit payload length in a usize, are you on 64bit architecture?");
      // return the rest of the bytes for deserialization
      ::std::slice::from_raw_parts(#variable, elements)
    };
    // deserialize, skipping the known 8 byte length field
    ::membrane::bincode::deserialize::<#ty>(&data[8..]).expect(
      format!("Deserialization error at variable '{}' of type '{}'", #variable_name, #str_ty).as_str()
    )
  }
}
