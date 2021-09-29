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

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Contact {
  pub id: u64,
  pub name: String,
  pub status: Status,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Error {
  pub message: String,
}
