use std::default::Default;

use membrane::dart_enum;
use serde::{Deserialize, Serialize};

#[dart_enum(namespace = "accounts")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Status {
  Pending,
  Active,
}

#[dart_enum(namespace = "accounts")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Reports {
  None,
  Name(String),
  Reports(Box<Self>),
}

impl Default for Status {
  fn default() -> Self {
    Status::Pending
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Contact {
  pub id: i64,
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Filter(pub Vec<Match>);

#[derive(Debug, Deserialize, Serialize)]
pub struct Match {
  field: String,
  value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contacts {
  pub data: Vec<Contact>,
  pub count: i32,
  pub total: i32,
}

#[derive(Deserialize, Serialize)]
pub struct SyncContacts(pub Vec<Contact>);

#[derive(Debug, Deserialize, Serialize)]
pub struct VecWrapper {
  data: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct MoreTypes {
  pub string: String,
  pub unsigned_8: u8,
  pub unsigned_16: u16,
  pub unsigned_32: u32,
  pub unsigned_64: u64,
  pub signed_8: i8,
  pub signed_16: i16,
  pub signed_32: i32,
  pub signed_64: i64,
  pub unsigned_128_min: u128,
  pub unsigned_128_64: u128,
  pub unsigned_128_max: u128,
  pub signed_128_min: i128,
  pub signed_128_64: i128,
  pub signed_128_neg_64: i128,
  pub signed_128_max: i128,
  pub float_32: f32,
  pub float_64: f64,
  #[serde(with = "serde_bytes")]
  pub blob: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
pub struct Location {
  pub polyline_coords: Vec<(f64, f64)>,
}
