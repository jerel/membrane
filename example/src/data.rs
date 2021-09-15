use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Contact {
  pub id: u32,
  pub name: String,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Error {
  message: String,
}
