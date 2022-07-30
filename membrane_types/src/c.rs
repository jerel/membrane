use crate::{rust::flatten_types, Input};

pub struct CHeaderTypes(Vec<String>);

impl std::convert::TryFrom<&Vec<Input>> for CHeaderTypes {
  type Error = syn::Error;

  fn try_from(inputs: &Vec<Input>) -> Result<Self, Self::Error> {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "{c_type}{variable}",
        c_type = c_type(
          &flatten_types(&input.ty, vec![])?
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<&str>>(),
          &input.ty
        )?,
        variable = &input.variable,
      ))
    }

    Ok(Self(stream))
  }
}

impl From<CHeaderTypes> for Vec<String> {
  fn from(types: CHeaderTypes) -> Self {
    types.0
  }
}

fn c_type(ty: &[&str], type_: &syn::Type) -> syn::Result<String> {
  let type_ = match ty[..] {
    ["String"] => "const char *",
    ["i64"] => "const int64_t ",
    ["f64"] => "const double ",
    ["bool"] => "const uint8_t ",
    ["Vec", ..] => "const uint8_t *",
    [serialized, ..] if serialized != "Option" => "const uint8_t *",
    ["Option", "String"] => "const char *",
    ["Option", "i64"] => "const int64_t *",
    ["Option", "f64"] => "const double *",
    ["Option", "bool"] => "const uint8_t *",
    ["Option", _serialized] => "const uint8_t *",
    _ => {
      return Err(syn::Error::new_spanned(
        type_,
        "not a supported argument type for Dart interop",
      ))
    }
  }
  .to_string();

  Ok(type_)
}
