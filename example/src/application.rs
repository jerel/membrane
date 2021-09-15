use membrane::async_dart;
use tokio_stream::Stream;

use crate::data;

#[async_dart(namespace = "accounts")]
pub fn contacts() -> impl Stream<Item = Result<data::Contact, data::Error>> {
  futures::stream::iter(vec![Ok(Default::default())])
}

#[async_dart(namespace = "accounts")]
pub async fn contact(id: String) -> Result<data::Contact, data::Error> {
  Ok(data::Contact {
    id: id.parse().unwrap(),
    ..Default::default()
  })
}
