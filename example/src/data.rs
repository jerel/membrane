use std::default::Default;

use membrane::dart_enum;
use serde::{Deserialize, Serialize};

#[dart_enum(namespace = "accounts")]
#[derive(Debug, Deserialize, Serialize)]
pub enum Status {
  Pending,
  Active,
}

impl Default for Status {
  fn default() -> Self {
    Status::Pending
  }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contact {
  pub id: u64,
  pub full_name: String,
  pub status: Status,
}

impl Default for Contact {
  fn default() -> Self {
    Self {
      id: 1,
      full_name: "Alice Smith".to_string(),
      status: Status::Pending,
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Error {
  pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OptionsDemo {
  pub one: Option<String>,
  pub two: Option<i64>,
  pub three: Option<f64>,
  pub four: Option<bool>,
  pub five: Option<Arg>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Arg {
  pub value: i64,
}
