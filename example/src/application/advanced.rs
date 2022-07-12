use data::OptionsDemo;
use membrane::emitter::{emitter, Emitter, StreamEmitter};
use membrane::{async_dart, sync_dart};
use tokio_stream::Stream;

// used for background threading examples
use std::{thread, time::Duration};

use crate::data::{self, MoreTypes};

#[async_dart(namespace = "accounts")] // also demonstrate fully qualified return types
pub async fn contact_panic() -> ::std::result::Result<data::Contact, data::Error> {
  panic!("The rust code panicked");
}

#[async_dart(namespace = "accounts", timeout = 20)]
pub fn contact_stream_panic(
) -> impl ::futures::stream::Stream<Item = Result<data::Contact, data::Error>> {
  panic!("The rust code panicked");
  #[allow(unreachable_code)]
  futures::stream::iter(vec![Ok(data::Contact::default())])
}

#[sync_dart(namespace = "accounts")]
pub fn contact_sync_panic() -> Result<data::Contact, data::Error> {
  panic!("The sync rust code panicked");
}

#[sync_dart(namespace = "accounts")]
pub fn contact_sync(count: i64) -> Result<data::SyncContacts, data::Error> {
  Ok(data::SyncContacts(
    (0..count).map(|_| data::Contact::default()).collect(),
  ))
}

#[async_dart(namespace = "accounts", os_thread = true)]
pub async fn contact_os_thread(user_id: String) -> Result<data::Contact, data::Error> {
  println!("os thread {:?}", thread::current().id());
  Ok(data::Contact {
    id: user_id.parse().unwrap(),
    ..data::Contact::default()
  })
}

#[async_dart(namespace = "accounts")]
pub fn contact_async_emitter(user_id: String) -> impl Emitter<Result<data::Contact, data::Error>> {
  let emitter = emitter!();

  print!(
    "\n[contact_async_emitter] sync Rust function {:?}",
    thread::current().id()
  );

  let contact = Ok(data::Contact {
    id: user_id.parse().unwrap(),
    ..data::Contact::default()
  });

  let e = emitter.clone();
  // drop the JoinHandle to detach the thread
  let _ = thread::spawn(move || {
    e.on_done(|| {
      println!("\n[contact_async_emitter] the finalizer has been called for the Emitter");
    });

    print!(
      "\n[contact_async_emitter] spawned thread is starting {:?}",
      thread::current().id()
    );

    println!(
      "[contact_async_emitter] Emitter state is {:?}",
      e.push(contact)
    );
    println!("\n[contact_async_emitter] spawned thread has sent response");

    print!(
      "[contact_async_emitter] spawned thread is finished and waiting to be cancelled by Dart"
    );
    let mut waiting = true;
    while waiting {
      waiting = !e.is_done();
      print!(".");
    }

    // this will result in an error because Dart has cancelled us
    println!(
      "[contact_async_emitter] Emitter state is {:?}",
      e.push(Ok(data::Contact::default()))
    );
  });

  print!("\n[contact_async_emitter] sync Rust function is returning");

  emitter
}

#[async_dart(namespace = "accounts")]
pub fn contact_async_stream_emitter(
  _user_id: String,
) -> impl StreamEmitter<Result<data::Contact, data::Error>> {
  let stream = emitter!();

  stream.on_done(move || {
    println!("[contact_async_stream_emitter] the finalizer has been called for the StreamEmitter");
  });

  println!(
    "\n[contact_async_stream_emitter] sync Rust function {:?}",
    thread::current().id()
  );

  [1, 2, 3]
    .iter()
    .map(|user_id| data::Contact {
      id: *user_id,
      ..data::Contact::default()
    })
    .for_each(|contact| {
      let stream = stream.clone();

      // drop the JoinHandle to detach the thread
      let _ = thread::spawn(move || {
        let id = thread::current().id();

        println!(
          "\n[contact_async_stream_emitter] spawned thread is starting {:?}",
          id
        );

        if contact.id > 2 {
          // sleep momentarily and let Dart cancel the stream
          // after it has received the 2 items the test requires
          thread::sleep(Duration::from_millis(10));
        }

        println!(
          "\n[contact_async_stream_emitter] Stream {:?} send state is {:?}",
          id,
          stream.push(Ok(contact))
        );

        println!(
          "\n[contact_async_stream_emitter] spawned thread {:?} has sent response, shutting down",
          id
        );
      });
    });

  print!("\n[contact_async_stream_emitter] sync Rust function is returning");

  stream
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

#[async_dart(namespace = "accounts", disable_logging = true)]
pub async fn scalar_empty() -> Result<(), String> {
  Ok(())
}

#[async_dart(namespace = "accounts")]
pub async fn scalar_i32(val: i64) -> Result<i32, String> {
  use std::convert::TryInto;

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
pub async fn vec(v: data::VecWrapper) -> Result<data::VecWrapper, String> {
  Ok(v)
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
    blob: vec![104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100],
  };

  assert!(types.blob == b"hello world");
  assert!(return_value == types);

  Ok(return_value)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_struct(values: Vec<data::Contact>) -> Result<Vec<data::Contact>, String> {
  println!("\n[Rust] Received struct vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_string(values: Vec<String>) -> Result<Vec<String>, String> {
  println!("\n[Rust] Received string vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_bool(values: Vec<bool>) -> Result<Vec<bool>, String> {
  println!("\n[Rust] Received bool vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_int(values: Vec<i64>) -> Result<Vec<i64>, String> {
  println!("\n[Rust] Received int vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_float(values: Vec<f64>) -> Result<Vec<f64>, String> {
  println!("\n[Rust] Received float vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_vec(values: Vec<Vec<i64>>) -> Result<Vec<Vec<i64>>, String> {
  println!("\n[Rust] Received nested vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_vec_option(
  values: Vec<Vec<Option<i64>>>,
) -> Result<Vec<Vec<Option<i64>>>, String> {
  println!("\n[Rust] Received nested vec of options: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_option_struct(
  values: Vec<Option<data::Contact>>,
) -> Result<Vec<Option<data::Contact>>, String> {
  println!("\n[Rust] Received optional struct vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_option_string(values: Vec<Option<String>>) -> Result<Vec<Option<String>>, String> {
  println!("\n[Rust] Received string option vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_option_bool(values: Vec<Option<bool>>) -> Result<Vec<Option<bool>>, String> {
  println!("\n[Rust] Received bool option vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_option_int(values: Vec<Option<i64>>) -> Result<Vec<Option<i64>>, String> {
  println!("\n[Rust] Received int option vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn vec_option_float(values: Vec<Option<f64>>) -> Result<Vec<Option<f64>>, String> {
  println!("\n[Rust] Received float option vec: {:?}", values);

  Ok(values)
}

#[async_dart(namespace = "accounts")]
pub async fn filter_arg(filter: data::Filter) -> Result<data::Contacts, String> {
  println!("\n[Rust] Received filter: {:?}", filter);

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

// test the handling of types with no path
use data::Status;

#[async_dart(namespace = "accounts")]
pub async fn optional_enum_arg(status: Option<Status>) -> Result<data::Contact, String> {
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
