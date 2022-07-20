use std::ffi::CString;

use futures::stream::Stream;
use futures::{StreamExt, TryStreamExt};
use membrane::async_dart;
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

// add a public function to this module... this prevents Rust prior to 1.60 from
// assuming this module is never used and stripping it out
pub fn load() {}

// This function and its types must match the C function that is called
extern "C" {
  pub fn print_via_c(arg1: *mut std::os::raw::c_char);
}

#[async_dart(namespace = "c_render")]
pub fn render_via_c() -> impl Stream<Item = Result<String, String>> {
  let mut count = 0;

  // typically you'd want to transform the error before handing it over to Dart
  // but for this simple example we'll just turn the std::io error into a string
  let server = server()
    .map_err(|err| err.to_string())
    .map_ok(|x| {
      // print _every_ item via C
      let c_string = CString::new(&*x).unwrap();
      let ptr = c_string.into_raw();
      unsafe {
        print_via_c(ptr);
      };

      x
    })
    .filter(move |_| {
      // only send the mod 10 items to Dart
      count = count + 1;
      if count % 10 == 0 {
        futures::future::ready(true)
      } else {
        futures::future::ready(false)
      }
    });

  // to keep things simple we'll spawn a task to start a client
  // instance... usually the client would be a separate program
  tokio::spawn(async {
    client().await.unwrap();
  });

  println!("[call_async_c] [Rust] finished with synchronous call to `call_async_c()`");

  server
}

async fn client() -> Result<(), std::io::Error> {
  // allow the client to bind any port on the loopback interface
  let sock = UdpSocket::bind("127.0.0.0:0").await?;
  let payload = "hello world".as_bytes();
  // send the length of the payload as the first byte
  // for this example we'll only send string lengths that fit in a u8
  let len = payload.len() as u8;
  let bytes = &vec![vec![len], payload.to_vec()].concat();

  println!("UDP client sending to 127.0.0.1:6000");

  loop {
    // pause a little to keep from spewing thousands of prints to the console
    sleep(Duration::from_millis(10)).await;
    let _len = sock.send_to(&bytes, "127.0.0.1:6000").await?;
  }
}

fn server() -> impl Stream<Item = Result<String, std::io::Error>> {
  async_stream::stream! {
    let sock = UdpSocket::bind("127.0.0.1:6000").await?;
    // create a 1024 byte buffer to read the string into, we'll send a max length of 256 bytes regardless
    let mut buf = [0; 1024];
    println!("UDP server listening on 127.0.0.1:6000");

    let mut count = 0;
    loop {
      count = count + 1;
      let (_len, _addr) = sock.recv_from(&mut buf).await?;
      let end_index: u8 = buf[0] + 1;
      let payload = buf[1..end_index as usize].to_vec();
      let data = format!("{} {}", String::from_utf8(payload).unwrap(), count);
      yield Ok(data);
    }
  }
}
