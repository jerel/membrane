use data::OptionsDemo;
use membrane::async_dart;
use tokio_stream::Stream;

use crate::data::{self, MoreTypes};

#[async_dart(namespace = "accounts")]
pub fn contacts() -> impl Stream<Item = Result<data::Contact, data::Error>> {
  futures::stream::iter(vec![Ok(data::Contact::default())])
}

#[async_dart(namespace = "accounts")]
pub async fn contact(user_id: String) -> Result<data::Contact, data::Error> {
  Ok(data::Contact {
    id: user_id.parse().unwrap(),
    ..data::Contact::default()
  })
}

#[async_dart(namespace = "accounts")]
pub async fn update_contact(
  id: String,
  contact: data::Contact,
  send_email: Option<bool>,
) -> Result<data::Contact, data::Error> {
  println!(
    "Rust received id {} with send_email flag {:?}: {:?}",
    id, send_email, contact
  );
  Ok(contact)
}

#[async_dart(namespace = "accounts")]
pub async fn delete_contact(id: String) -> Result<data::Contact, data::Error> {
  Err(data::Error {
    message: format!("{} cannot be deleted", id),
  })
}

#[async_dart(namespace = "accounts")]
pub async fn options_demo(
  one: Option<String>,
  two: Option<i64>,
  three: Option<f64>,
  four: Option<bool>,
  five: Option<data::Arg>,
) -> Result<OptionsDemo, data::Error> {
  Ok(OptionsDemo {
    one,
    two,
    three,
    four,
    five,
  })
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_i64(val: i64) -> Result<i64, String> {
  assert!(val == 10);
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_f64(val: f64) -> Result<f64, String> {
  assert!(val == 11.1);
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_string(val: String) -> Result<String, String> {
  assert!(val == "hello world");
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_bool(val: bool) -> Result<bool, String> {
  assert!(val == true);
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_error() -> Result<bool, String> {
  Err("an error message".to_string())
}

#[async_dart(namespace = "accounts")]
pub async fn more_types() -> Result<data::MoreTypes, String> {
  Ok(MoreTypes {
    one: 255,
    two: 100,
    three: u128::MIN,
    four: 200,
    five: u128::MAX,
    six: i128::MIN,
    seven: 300,
    eight: -300,
    nine: i128::MAX,
  })
}
