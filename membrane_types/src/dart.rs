use crate::Input;
use syn::Type;

pub struct DartParams(Vec<String>);
pub struct DartTransforms(Vec<String>);
pub struct DartArgs(Vec<String>);

impl From<&Vec<Input>> for DartParams {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "{dart_type} {variable}",
        variable = &input.variable,
        dart_type = dart_type(&input.rust_type)
      ))
    }

    Self(stream)
  }
}

impl From<&Vec<Input>> for DartTransforms {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "var c{variable} = {cast}",
        variable = &input.variable,
        cast = cast_dart_type_to_c(&input.rust_type, &input.variable, &input.ty)
      ))
    }

    Self(stream)
  }
}

impl From<&Vec<Input>> for DartArgs {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!("c{variable}", variable = &input.variable))
    }

    Self(stream)
  }
}

impl From<DartParams> for Vec<String> {
  fn from(types: DartParams) -> Self {
    types.0
  }
}

impl From<DartTransforms> for Vec<String> {
  fn from(types: DartTransforms) -> Self {
    types.0
  }
}

impl From<DartArgs> for Vec<String> {
  fn from(types: DartArgs) -> Self {
    types.0
  }
}

fn dart_type(str_ty: &str) -> String {
  match str_ty {
    "String" => "String",
    "i64" => "int",
    "f64" => "double",
    "bool" => "bool",
    _serialized => str_ty.split("::").last().unwrap().trim(),
  }
  .to_string()
}

fn cast_dart_type_to_c(str_ty: &str, variable: &str, ty: &Type) -> String {
  match ty {
    &syn::Type::Reference(_) => panic!("{}", unsupported_type_error(str_ty, variable, "a struct")),
    &syn::Type::Tuple(_) | &syn::Type::Slice(_) | &syn::Type::Array(_) => {
      panic!("{}", unsupported_type_error(str_ty, variable, "a struct"))
    }
    &syn::Type::Path(ref p) => match p.path.segments.first() {
      Some(segment) if segment.ident == "Vec" => {
        panic!("{}", unsupported_type_error(str_ty, variable, "a struct"))
      }
      _ => (),
    },
    _ => (),
  };

  match str_ty {
    "String" => {
      format!(
        "{variable}.toNativeUtf8().cast<Int8>()",
        variable = variable
      )
    }
    "bool" => format!("{variable} ? 1 : 0", variable = variable),
    "& str" => panic!("{}", unsupported_type_error(str_ty, variable, "String")),
    "char" => panic!("{}", unsupported_type_error(str_ty, variable, "String")),
    "i8" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i16" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i32" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i64" => format!("{variable}", variable = variable),
    "i128" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u8" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u16" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u32" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u64" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u128" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "f32" => panic!("{}", unsupported_type_error(str_ty, variable, "f64")),
    "f64" => format!("{variable}", variable = variable),
    _serialized => format!(
      r#"(){{
      final data = {variable}.bincodeSerialize();
      final blob = calloc<Uint8>(data.length + 8);
      final blobBytes = blob.asTypedList(data.length + 8);
      final payloadLength = Int64List(1);
      payloadLength.setAll(0, [data.length + 8]);
      blobBytes.setAll(0, payloadLength);
      blobBytes.setAll(8, data);
      return blob;
    }}()"#,
      variable = variable
    ),
  }
}

fn unsupported_type_error(ty: &str, variable: &str, new_ty: &str) -> String {
  format!(
    "A Rust type of {ty} is invalid for `{var}: {ty}`. Please use {new_ty} instead.",
    ty = ty.split_whitespace().collect::<String>(),
    var = variable,
    new_ty = new_ty
  )
}
