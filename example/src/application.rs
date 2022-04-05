use std::convert::TryInto;

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

//
// Functions below are used by integration tests
//

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

#[async_dart(namespace = "accounts", disable_logging = true)]
pub async fn scalar_empty() -> Result<(), String> {
  Ok(())
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_i32(val: i64) -> Result<i32, String> {
  assert!(val == 123);
  Ok(val.try_into().unwrap())
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_i64(val: i64) -> Result<i64, String> {
  assert!(val == 10);
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_f32(val: f64) -> Result<f32, String> {
  assert!(val == 21.1);
  Ok(21.1)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_f64(val: f64) -> Result<f64, String> {
  assert!(val == 11.1);
  Ok(val)
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_string(val: String) -> Result<String, String> {
  assert!(val == "hello world / ダミーテキスト");
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
pub async fn more_types(types: data::MoreTypes) -> Result<data::MoreTypes, String> {
  let return_value = MoreTypes {
    string: "hello world / ダミーテキスト".to_string(),
    unsigned_8: u8::MAX,
    unsigned_16: u16::MAX,
    unsigned_32: u32::MAX,
    unsigned_64: u64::MAX,
    signed_8: i8::MAX,
    signed_16: i16::MAX,
    signed_32: i32::MAX,
    signed_64: i64::MAX,
    unsigned_128_min: u128::MIN,
    unsigned_128_64: 200,
    unsigned_128_max: u128::MAX,
    signed_128_min: i128::MIN,
    signed_128_64: 300,
    signed_128_neg_64: -300,
    signed_128_max: i128::MAX,
    float_32: 3.140000104904175,
    float_64: f64::MAX,
  };

  assert!(return_value == types);

  Ok(return_value)
}

#[async_dart(namespace = "accounts")]
pub async fn filter_arg(filter: data::Filter) -> Result<data::Contacts, String> {
  println!("[Rust] Received filter: {:?}", filter);

  Ok(data::Contacts {
    data: vec![data::Contact::default()],
    count: 1,
    total: 1,
  })
}

#[async_dart(namespace = "accounts")]
pub async fn enum_arg(status: data::Status) -> Result<data::Contact, String> {
  Ok(data::Contact {
    status,
    ..data::Contact::default()
  })
}

#[async_dart(namespace = "accounts")]
pub async fn optional_enum_arg(status: Option<data::Status>) -> Result<data::Contact, String> {
  match status {
    Some(status) => Ok(data::Contact {
      status,
      ..data::Contact::default()
    }),
    _ => Ok(data::Contact {
      ..data::Contact::default()
    }),
  }
}

#[async_dart(namespace = "accounts")]
pub async fn enum_return(status: data::Status) -> Result<data::Status, String> {
  Ok(status)
}

#[async_dart(namespace = "accounts", timeout = 100)]
pub async fn slow_function(sleep_for: i64) -> Result<(), String> {
  use tokio::time::{sleep, Duration};
  sleep(Duration::from_millis(sleep_for as u64)).await;
  Ok(())
}

#[async_dart(namespace = "accounts")]
pub async fn slow_function_two(sleep_for: i64) -> Result<(), String> {
  use tokio::time::{sleep, Duration};
  sleep(Duration::from_millis(sleep_for as u64)).await;
  Ok(())
}

#[async_dart(namespace = "accounts", timeout = 50)]
pub fn slow_stream(sleep_for: i64) -> impl Stream<Item = Result<i32, String>> {
  use async_stream::stream;
  use tokio::time::{sleep, Duration};

  stream! {
    for i in 0..3 {
      sleep(Duration::from_millis(sleep_for as u64)).await;
      yield Ok(i);
    }
  }
}

#[async_dart(namespace = "locations")]
pub async fn get_location(id: i64) -> Result<data::Location, String> {
  let _id = id;

  Ok(data::Location {
    polyline_coords: vec![
      (-104.0185546875, 43.004647127794435),
      (-104.0625, 37.78808138412046),
      (-94.130859375, 37.85750715625203),
    ],
  })
}
