use crate::{rust::flatten_types, Input};
use heck::{CamelCase, MixedCase};
use syn::Type;

pub struct DartParams(Vec<String>);
pub struct DartTransforms(Vec<String>);
pub struct DartArgs(Vec<String>);

impl std::convert::TryFrom<&Vec<Input>> for DartParams {
  type Error = syn::Error;

  fn try_from(inputs: &Vec<Input>) -> Result<Self, Self::Error> {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "{dart_type} {variable}",
        dart_type = dart_param_type(
          &flatten_types(&input.ty, vec![])?
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<&str>>(),
          &input.ty
        )?,
        variable = &input.variable.to_mixed_case(),
      ))
    }

    Ok(Self(stream))
  }
}

impl std::convert::TryFrom<&Vec<Input>> for DartTransforms {
  type Error = syn::Error;

  fn try_from(inputs: &Vec<Input>) -> Result<Self, Self::Error> {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "final c{variable} = {cast}",
        variable = &input.variable.to_camel_case(),
        cast = cast_dart_type_to_c(
          &flatten_types(&input.ty, vec![])?
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<&str>>(),
          &input.variable,
          &input.ty
        )?
      ))
    }

    Ok(Self(stream))
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

pub fn dart_type(types: &[&str]) -> String {
  let ty;
  match types[..] {
    ["String"] => "String",
    ["i8"] => "int",
    ["u8"] => "int",
    ["i16"] => "int",
    ["u16"] => "int",
    ["i32"] => "int",
    ["u32"] => "int",
    ["i64"] => "int",
    ["u64"] => "Uint64",
    ["i128"] => "Int128",
    ["u128"] => "Uint128",
    ["f32"] => "double",
    ["f64"] => "double",
    ["bool"] => "bool",
    ["()"] => "void",
    ["Option", ..] => {
      ty = format!("{}?", dart_type(&types[1..]));
      &ty
    }
    ["Vec", "Option", ..] => {
      ty = format!("List<{}?>", dart_type(&types[2..]));
      &ty
    }
    ["Vec", ..] => {
      ty = format!("List<{}>", dart_type(&types[1..]));
      &ty
    }
    _ => types[0],
  }
  .to_string()
}

fn dart_param_type(types: &[&str], type_: &syn::Type) -> syn::Result<String> {
  let ty;
  let result = match types[..] {
    ["String"] => "required String",
    ["i64"] => "required int",
    ["f64"] => "required double",
    ["bool"] => "required bool",
    ["Vec", "Option", ..] => {
      ty = format!("required List<{}?>", dart_type(&types[2..]));
      &ty
    }
    ["Vec", ..] => {
      ty = format!("required List<{}>", dart_type(&types[1..]));
      &ty
    }
    [serialized] if serialized != "Option" => {
      ty = format!("required {} ", serialized);
      &ty
    }
    ["Option", "String"] => "String?",
    ["Option", "i64"] => "int?",
    ["Option", "f64"] => "double?",
    ["Option", "bool"] => "bool?",
    ["Option", ..] => {
      ty = format!("{}? ", dart_type(&types[1..]));
      &ty
    }
    _ => {
      return Err(syn::Error::new_spanned(
        type_,
        "not a supported argument type for Dart interop",
      ))
    }
  }
  .to_string();

  Ok(result)
}

fn cast_dart_type_to_c(types: &[&str], variable: &str, ty: &Type) -> syn::Result<String> {
  match ty {
    &syn::Type::Reference(_) => return unsupported_type_error(ty, "a struct"),
    &syn::Type::Tuple(_) | &syn::Type::Slice(_) | &syn::Type::Array(_) => {
      return unsupported_type_error(ty, "a struct")
    }
    _ => (),
  };

  let cast = match types[..] {
    ["&str"] => return unsupported_type_error(ty, "String"),
    ["char"] => return unsupported_type_error(ty, "String"),
    ["i8"] => return unsupported_type_error(ty, "i64"),
    ["i16"] => return unsupported_type_error(ty, "i64"),
    ["i32"] => return unsupported_type_error(ty, "i64"),
    ["i128"] => return unsupported_type_error(ty, "i64"),
    ["u8"] => return unsupported_type_error(ty, "i64"),
    ["u16"] => return unsupported_type_error(ty, "i64"),
    ["u32"] => return unsupported_type_error(ty, "i64"),
    ["u64"] => return unsupported_type_error(ty, "i64"),
    ["u128"] => return unsupported_type_error(ty, "i64"),
    ["f32"] => return unsupported_type_error(ty, "f64"),
    //
    // supported types
    //
    ["String"] => {
      format!(
        r#"(){{
          final ptr = {variable}.toNativeUtf8().cast<Char>();
          _toFree.add(ptr);
          return ptr;
        }}()"#,
        variable = variable.to_mixed_case()
      )
    }
    ["bool"] => format!("{variable} ? 1 : 0", variable = variable.to_mixed_case()),
    ["i64"] => variable.to_mixed_case(),
    ["f64"] => variable.to_mixed_case(),
    ["Vec", ..] => format!(
      r#"(){{
      final serializer = BincodeSerializer();
      {serializer}
      final data = serializer.bytes;
      {ser_partial}
    }}()"#,
      serializer = serializer(types, &variable.to_mixed_case(), ty)?,
      ser_partial = serialization_partial(),
    ),
    [ty, ..] if ty != "Option" => format!(
      r#"(){{
      final data = {variable}.bincodeSerialize();
      {ser_partial}
    }}()"#,
      variable = variable.to_mixed_case(),
      ser_partial = serialization_partial(),
    ),
    ["Option", "String"] => {
      format!(
        r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final ptr = {variable}.toNativeUtf8().cast<Char>();
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
    ["Option", "Vec", ..] => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final serializer = BincodeSerializer();
      {serializer}
      final data = serializer.bytes;
      {ser_partial}
    }}()"#,
      variable = variable.to_mixed_case(),
      serializer = serializer(types, &variable.to_mixed_case(), ty)?,
      ser_partial = serialization_partial(),
    ),
    ["Option", ..] => format!(
      r#"(){{
      if ({variable} == null) {{
        return nullptr;
      }}
      final data = {variable}.bincodeSerialize();
      {ser_partial}
    }}()"#,
      variable = variable.to_mixed_case(),
      ser_partial = serialization_partial(),
    ),
    _ => {
      return Err(syn::Error::new_spanned(
        ty,
        "not a supported argument type for Dart interop",
      ))
    }
  };

  Ok(cast)
}

fn unsupported_type_error(ty: &syn::Type, new_ty: &str) -> Result<String, syn::Error> {
  Err(syn::Error::new_spanned(
    ty,
    format!(
      "not a supported argument type for Dart interop, please use {new_ty} instead.",
      new_ty = new_ty
    ),
  ))
}

fn serialization_partial() -> &'static str {
  r#"final ptr = calloc<Uint8>(data.length + 8);
_toFree.add(ptr);
final blobBytes = ptr.asTypedList(data.length + 8);
blobBytes.buffer.asUint64List(0, 1)[0] = data.length + 8;
blobBytes.setAll(8, data);
return ptr;"#
}

fn serializer(types: &[&str], variable: &str, ty: &Type) -> Result<String, syn::Error> {
  match types[..] {
    ["i8"] => unsupported_type_error(ty, "i64"),
    ["i16"] => unsupported_type_error(ty, "i64"),
    ["i32"] => unsupported_type_error(ty, "i64"),
    ["u8"] => unsupported_type_error(ty, "i64"),
    ["u16"] => unsupported_type_error(ty, "i64"),
    ["u32"] => unsupported_type_error(ty, "i64"),
    ["f32"] => unsupported_type_error(ty, "f64"),
    ["String"] => Ok("serializer.serializeString(value)".to_string()),
    ["bool"] => Ok("serializer.serializeBool(value)".to_string()),
    ["i64"] => Ok("serializer.serializeInt64(value)".to_string()),
    ["u64"] => Ok("serializer.serializeUint64(value)".to_string()),
    ["i128"] => Ok("serializer.serializeInt128(value)".to_string()),
    ["u128"] => Ok("serializer.serializeUint128(value)".to_string()),
    ["f64"] => Ok("serializer.serializeFloat64(value)".to_string()),
    ["Vec", "Option", ..] => Ok(format!(
      "serializer.serializeLength({variable}.length);
      {variable}.forEach((value) {{
        serializer.serializeOptionTag(value != null);
        if (value != null) {{
          {serializer};
        }}
      }});",
      variable = variable,
      serializer = serializer(&types[2..], "value", ty)?,
    )),
    ["Vec", ..] => Ok(format!(
      "serializer.serializeLength({variable}.length);
      {variable}.forEach((value) {{
        {serializer};
      }});",
      variable = variable,
      serializer = serializer(&types[1..], "value", ty)?,
    )),
    ["Option", ..] => {
      Ok(format!(
        // the containing serialization code does an early return if the
        // value is null so we don't have to do a null check here
        "{serializer};",
        serializer = serializer(&types[1..], variable, ty)?,
      ))
    }
    _ => Ok("value.serialize(serializer)".to_string()),
  }
}
