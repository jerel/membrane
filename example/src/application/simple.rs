use membrane::async_dart;
use tokio_stream::Stream;

use std::thread;

use crate::data;

#[async_dart(namespace = "accounts")]
pub fn contacts() -> impl Stream<Item = Result<data::Contact, data::Error>> {
  futures::stream::iter(vec![Ok(data::Contact::default())])
}

///
/// This is a docblock that was written in Rust and
/// will be added to the generated Dart code.
///
#[async_dart(namespace = "accounts")]
pub async fn contact(user_id: String) -> Result<data::Contact, data::Error> {
  println!("async {:?}", thread::current().id());
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
