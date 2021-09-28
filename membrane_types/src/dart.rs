use crate::Input;
use heck::{CamelCase, MixedCase};
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
        dart_type = dart_type(&input.rust_type),
        variable = &input.variable.to_mixed_case(),
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
        "final c{variable} = {cast}",
        variable = &input.variable.to_camel_case(),
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
      stream.push(format!(
        "c{variable}",
        variable = &input.variable.to_camel_case()
      ))
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
  let ser_type;
  match str_ty {
    "String" => "required String",
    "i64" => "required int",
    "f64" => "required double",
    "bool" => "required bool",
    serialized if !serialized.starts_with("Option<") => {
      ser_type = format!("required {} ", str_ty.split("::").last().unwrap().trim());
      &ser_type
    }
    "Option<String>" => "String?",
    "Option<i64>" => "int?",
    "Option<f64>" => "double?",
    "Option<bool>" => "bool?",
    serialized if serialized.starts_with("Option<") => {
      ser_type = format!(
        "{}? ",
        str_ty
          .split("::")
          .last()
          .unwrap()
          .trim_end_matches(|c| c == ' ' || c == '>')
      );
      &ser_type
    }
    _ => unreachable!(),
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
    "&str" => panic!("{}", unsupported_type_error(str_ty, variable, "String")),
    "char" => panic!("{}", unsupported_type_error(str_ty, variable, "String")),
    "i8" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i16" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i32" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "i128" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u8" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u16" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u32" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u64" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "u128" => panic!("{}", unsupported_type_error(str_ty, variable, "i64")),
    "f32" => panic!("{}", unsupported_type_error(str_ty, variable, "f64")),
    //
    // supported types
    //
    "String" => {
      format!(
        "{variable}.toNativeUtf8().cast<Int8>()",
        variable = variable.to_mixed_case()
      )
    }
    "bool" => format!("{variable} ? 1 : 0", variable = variable.to_mixed_case()),
    "i64" => format!("{variable}", variable = variable.to_mixed_case()),
    "f64" => format!("{variable}", variable = variable.to_mixed_case()),
    serialized if !serialized.starts_with("Option<") => format!(
      r#"(){{
      final data = {variable}.bincodeSerialize();
      {serialize}
    }}()"#,
      variable = variable.to_mixed_case(),
      serialize = serialization_partial(),
    ),
    "Option<String>" => {
      format!(
        r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      return {variable}.toNativeUtf8().cast<Int8>();
    }}()"#,
        variable = variable.to_mixed_case()
      )
    }
    "Option<bool>" => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Uint8>();
      ptr.asTypedList(1).setAll(0, [{variable} ? 1 : 0]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    "Option<i64>" => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Int64>();
      ptr.asTypedList(1).setAll(0, [{variable}]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    "Option<f64>" => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Double>();
      ptr.asTypedList(1).setAll(0, [{variable}]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    serialized if serialized.starts_with("Option<") => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final data = {variable}.bincodeSerialize();
      {serialize}
    }}()"#,
      variable = variable.to_mixed_case(),
      serialize = serialization_partial(),
    ),
    _ => unreachable!(),
  }
}

fn unsupported_type_error(ty: &str, variable: &str, new_ty: &str) -> String {
  format!(
    "A Rust type of {ty} is invalid for `{var}: {ty}`. Please use {new_ty} instead.",
    ty = ty,
    var = variable,
    new_ty = new_ty
  )
}

fn serialization_partial() -> &'static str {
  r#"final blob = calloc<Uint8>(data.length + 8);
final blobBytes = blob.asTypedList(data.length + 8);
final payloadLength = Int64List(1);
payloadLength.setAll(0, [data.length + 8]);
blobBytes.setAll(0, payloadLength);
blobBytes.setAll(8, data);
return blob;"#
}
