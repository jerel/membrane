use membrane::async_dart;
use membrane::emitter::{emitter, CHandle, Emitter, StreamEmitter};

// add a public function to this module... this prevents Rust prior to 1.60 from
// assuming this module is never used and stripping it out
pub fn load() {}

// This function and its types must match the C function that is called
extern "C" {
  pub fn init(arg1: CHandle) -> ::std::os::raw::c_int;
}

#[async_dart(namespace = "c_example")]
pub fn call_async_c() -> impl StreamEmitter<Result<String, String>> {
  let stream = emitter!();

  let s = stream.clone();
  let handle = stream.on_data(move |data: &std::os::raw::c_char| {
    let c_data = unsafe { std::ffi::CStr::from_ptr(data).to_owned() };

    let result = match c_data.into_string().into() {
      Ok(val) => Ok(val),
      Err(std::ffi::IntoStringError { .. }) => Err("Couldn't convert to a String".to_string()),
    };

    let _ = s.push(result.clone());
  });

  stream.on_done(|| {
    println!("[call_async_c] [Rust] stream is closed by Dart");
  });

  unsafe {
    // call into C to kick off the async work
    init(handle);
  }

  println!("[call_async_c] [Rust] finished with synchronous call to `call_async_c()`");

  stream
}
