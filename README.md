# Membrane

Membrane is an opinionated crate that generates a Dart package from a Rust library. Extremely fast performance with strict typing and zero copy returns over the FFI boundary via bincode.

## Development Environment

* Rust
  * https://rustup.rs
* Dart
  * https://dart.dev/get-dart
* libclang (for generating bindings)
  * Linux
    * `apt-get install libclang-dev`
  * MacOS
    * `brew install llvm@11`

## Example

First create a `lib.rs` that exposes a `RUNTIME` static that will survive for the lifetime of the program. `RUNTIME` must provide a tokio style `spawn` function:
``` rust
use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
  Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("libexample")
    .build()
    .unwrap()
});
```

Then write some code that is annotated with the `#[async_dart]` macro. The functions can be anywhere in your program and may return either an async `Result<T, E>` or a `Stream<Item = Result<T, E>>`:

``` rust
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
```

And now you are ready to generate the Dart package. Note that this goes in a `bin/generator.rs` or similar to be ran with `cargo run` rather than in a `build.rs` (which only runs before compilation):

``` rust
fn main() {
  // if nothing else in this generator.rs references lib.rs then
  // at least call a dummy function so lib.rs doesn't get optimized away
  example::load();

  let mut project = membrane::Membrane::new();
  project
    // name the output pub package
    .package_destination_dir("../dart_example")
    // give the name of the .so or .dylib that your Rust program provides
    .using_lib("libexample")
    .create_pub_package()
    .write_api()
    .write_c_headers()
    .write_bindings();
}
```

If everything went as planned you can now call Rust from Dart with:

``` bash
cd example &&
cargo build &&
cd ../dart_example &&
cp ../example/target/debug/libexample.dylib . &&
dart --enable-asserts run
```
(`--enable-asserts` enables a pretty print `toString()` in the generated classes)

``` dart
import 'package:dart_example/accounts.dart';

void main(List<String> arguments) async {
  var accounts = AccountsApi();
  print(await accounts.contact(id: "1"));
}
```
