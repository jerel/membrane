use std::env;
use std::path::PathBuf;
use toml::Value;

//
// Fetch the crate type from the Cargo.toml of the currently-compiling crate
//
pub fn get_lib_type() -> Vec<String> {
  let toml = std::fs::read_to_string(
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml"),
  )
  .unwrap();

  match toml::from_str::<Value>(&toml) {
    Ok(Value::Table(table)) => match table.get("lib") {
      Some(Value::Table(lib)) => match lib.get("crate-type") {
        Some(Value::Array(crate_type)) => crate_type
          .iter()
          .map(|x| match x {
            Value::String(val) => val.to_string(),
            _ => String::new(),
          })
          .collect::<Vec<String>>(),
        _ => vec![],
      },
      _ => vec![],
    },
    _ => vec![],
  }
}