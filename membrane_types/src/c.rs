use crate::Input;

pub struct CHeaderTypes(Vec<String>);

impl From<&Vec<Input>> for CHeaderTypes {
  fn from(inputs: &Vec<Input>) -> Self {
    let mut stream = vec![];

    for input in inputs {
      stream.push(format!(
        "{c_type}{variable}",
        c_type = c_type(&input.rust_type),
        variable = &input.variable,
      ))
    }

    Self(stream)
  }
}

impl From<CHeaderTypes> for Vec<String> {
  fn from(types: CHeaderTypes) -> Self {
    types.0
  }
}

fn c_type(ty: &str) -> String {
  match ty {
    "String" => "const char *",
    "i64" => "const signed long ",
    "f64" => "const double ",
    "bool" => "const uint8_t ",
    serialized if !serialized.starts_with("Option<") => "const uint8_t *",
    "Option<String>" => "const char *",
    "Option<i64>" => "const signed long *",
    "Option<f64>" => "const double *",
    "Option<bool>" => "const uint8_t *",
    serialized if serialized.starts_with("Option<") => "const uint8_t *",
    _ => unreachable!(),
  }
  .to_string()
}
