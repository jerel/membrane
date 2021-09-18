use crate::Input;

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
        cast = cast_dart_type_to_c(&input.rust_type, &input.variable)
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

fn dart_type(ty: &str) -> String {
  match ty {
    "String" => "String",
    "i64" => "int",
    "f64" => "double",
    "bool" => "bool",
    _serialized => ty.split("::").last().unwrap().trim(),
  }
  .to_string()
}

fn cast_dart_type_to_c(ty: &str, variable: &str) -> String {
  match ty {
    "String" => {
      format!(
        "{variable}.toNativeUtf8().cast<Int8>()",
        variable = variable
      )
    }
    "i64" => format!("{variable}", variable = variable),
    "f64" => format!("{variable}", variable = variable),
    "bool" => format!("{variable} ? 1 : 0", variable = variable),
    _serialized => format!(
      r#"(){{
      final data = {variable}.bincodeSerialize();
      final blob = calloc<Uint8>(data.length + 8);
      final blobBytes = blob.asTypedList(data.length + 8);
      final payloadLength = Int64List(1);
      payloadLength.setAll(0, [data.length + 8]);
      blobBytes.setAll(0, payloadLength);
      blobBytes.setAll(8, data);
      print(blobBytes.buffer.asInt8List());
      return blob;
    }}()"#,
      variable = variable
    ),
  }
}
