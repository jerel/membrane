use crate::{rust::flatten_types, Input};
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
        dart_type = dart_type(
          &flatten_types(&input.ty, vec![])
            .iter()
            .map(|x| x.as_str())
            .collect()
        ),
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
        cast = cast_dart_type_to_c(
          &flatten_types(&input.ty, vec![])
            .iter()
            .map(|x| x.as_str())
            .collect(),
          &input.variable,
          &input.ty
        )
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

pub fn dart_bare_type<'a>(str_ty: &Vec<&'a str>) -> String {
  let tmp;
  match str_ty[..] {
    ["String"] => "String",
    ["i32"] => "int",
    ["i64"] => "int",
    ["f32"] => "double",
    ["f64"] => "double",
    ["bool"] => "bool",
    ["()"] => "void",
    ["Vec", "Option", ..] => {
      tmp = format!("List<{}?>", dart_bare_type(&str_ty[2..].to_vec()));
      &tmp
    }
    ["Vec", ..] => {
      tmp = format!("List<{}>", dart_bare_type(&str_ty[1..].to_vec()));
      &tmp
    }
    _ => str_ty[0],
  }
  .to_string()
}

fn dart_type<'a>(str_ty: &Vec<&str>) -> String {
  let ser_type;
  match str_ty[..] {
    ["String"] => "required String",
    ["i64"] => "required int",
    ["f64"] => "required double",
    ["bool"] => "required bool",
    ["Vec", ty] => {
      ser_type = format!("required List<{}>", dart_bare_type(&vec![ty]));
      &ser_type
    }
    ["Vec", "Option", ty] => {
      ser_type = format!("required List<{}?>", dart_bare_type(&vec![ty]));
      &ser_type
    }
    [serialized] if serialized != "Option" => {
      ser_type = format!("required {} ", serialized);
      &ser_type
    }
    ["Option", "String"] => "String?",
    ["Option", "i64"] => "int?",
    ["Option", "f64"] => "double?",
    ["Option", "bool"] => "bool?",
    ["Option", serialized] => {
      ser_type = format!("{}? ", serialized);
      &ser_type
    }
    _ => unreachable!("[dart_type] macro checks should make this code unreachable"),
  }
  .to_string()
}

fn cast_dart_type_to_c(str_ty: &Vec<&str>, variable: &str, ty: &Type) -> String {
  match ty {
    &syn::Type::Reference(_) => panic!(
      "{}",
      unsupported_type_error(str_ty[0], variable, "a struct")
    ),
    &syn::Type::Tuple(_) | &syn::Type::Slice(_) | &syn::Type::Array(_) => {
      panic!(
        "{}",
        unsupported_type_error(str_ty[0], variable, "a struct")
      )
    }
    _ => (),
  };

  match str_ty[..] {
    ["&str"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "String")),
    ["char"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "String")),
    ["i8"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["i16"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["i32"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["i128"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["u8"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["u16"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["u32"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["u64"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["u128"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "i64")),
    ["f32"] => panic!("{}", unsupported_type_error(str_ty[0], variable, "f64")),
    //
    // supported types
    //
    ["String"] => {
      format!(
        r#"(){{
          final ptr = {variable}.toNativeUtf8().cast<Int8>();
          _toFree.add(ptr);
          return ptr;
        }}()"#,
        variable = variable.to_mixed_case()
      )
    }
    ["bool"] => format!("{variable} ? 1 : 0", variable = variable.to_mixed_case()),
    ["i64"] => variable.to_mixed_case(),
    ["f64"] => variable.to_mixed_case(),
    ["Vec", ty] => format!(
      r#"(){{
      final serializer = BincodeSerializer();
      serializer.serializeLength({variable}.length);
      for (final value in {variable}) {{
        {serializer};
      }}
      final data = serializer.bytes;
      {serialize}
    }}()"#,
      variable = variable.to_mixed_case(),
      serializer = serializer(ty),
      serialize = serialization_partial(),
    ),
    ["Vec", "Option", ty] => format!(
      r#"(){{
      final serializer = BincodeSerializer();
      serializer.serializeLength({variable}.length);
      for (final value in {variable}) {{
        serializer.serializeOptionTag(value != null);
        if (value != null) {{
          {serializer};
        }}
      }}
      final data = serializer.bytes;
      {serialize}
    }}()"#,
      variable = variable.to_mixed_case(),
      serializer = serializer(ty),
      serialize = serialization_partial(),
    ),
    [ty, ..] if ty != "Option" => format!(
      r#"(){{
      final data = {variable}.bincodeSerialize();
      {serialize}
    }}()"#,
      variable = variable.to_mixed_case(),
      serialize = serialization_partial(),
    ),
    ["Option", "String"] => {
      format!(
        r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = {variable}.toNativeUtf8().cast<Int8>();
      _toFree.add(ptr);
      return ptr;
    }}()"#,
        variable = variable.to_mixed_case()
      )
    }
    ["Option", "bool"] => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Uint8>();
      _toFree.add(ptr);
      ptr.asTypedList(1).setAll(0, [{variable} ? 1 : 0]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    ["Option", "i64"] => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Int64>();
      _toFree.add(ptr);
      ptr.asTypedList(1).setAll(0, [{variable}]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    ["Option", "f64"] => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = calloc<Double>();
      _toFree.add(ptr);
      ptr.asTypedList(1).setAll(0, [{variable}]);
      return ptr;
    }}()"#,
      variable = variable.to_mixed_case()
    ),
    ["Option", ..] => format!(
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
    _ => unreachable!("[cast_dart_type_to_c] macro checks should make this code unreachable"),
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
  r#"final ptr = calloc<Uint8>(data.length + 8);
_toFree.add(ptr);
final blobBytes = ptr.asTypedList(data.length + 8);
blobBytes.buffer.asUint64List(0, 1)[0] = data.length + 8;
blobBytes.setAll(8, data);
return ptr;"#
}

fn serializer(str_ty: &str) -> &'static str {
  match str_ty {
    "String" => "serializer.serializeString(value)",
    "bool" => "serializer.serializeBool(value)",
    "i64" => "serializer.serializeInt64(value)",
    "f64" => "serializer.serializeFloat64(value)",
    _ => "value.serialize(serializer)",
  }
}
