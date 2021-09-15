# Membrane

Generate an opinionated Dart package from a Rust library. Extremely fast performance with strict typing and zero copy over the FFI boundary.

## Example

First create a lib.rs that exposes a `RUNTIME` static that will survive for the lifetime of the program:
``` rust
use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

pub(crate) static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
  Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("libacme")
    .build()
    .unwrap()
});
```

Then write some code that is annotated with the `#[async_dart]` macro. The functions can be anywhere in your program and may return either an async `Result<T, E>` or a `Stream<Result<T, E>>`:

``` rust
use membrane::async_dart;
use tokio_stream::Stream;

#[async_dart(namespace = "accounts")]
pub fn contacts() -> impl Stream<Item = Result<account::Contact, account::Error>> {
  futures::stream::iter(vec![Ok(account::Contact::new())])
}

#[async_dart(namespace = "accounts")]
pub async fn contact(id: String) -> Result<account::Contact, account::Error> {
  Ok(account::Contact::new())
}
```

And now you are ready to generate the Dart package. Note that this goes in a `bin.rs` to be ran with `cargo run` as part of compilation rather than in a `build.rs` which is just before compilation:

``` rust
fn main() {
  // if nothing else in this bin.rs references lib.rs then
  // at least call a dummy function so lib.rs doesn't get optimized away
  acme::load();

  let mut project = membrane::Membrane::new();
  project
    // name the output pub package
    .package_destination_dir("../dart_acme")
    // give the name of the .so or .dylib that your Rust program provides
    .using_lib("libacme")
    .create_pub_package()
    .write_api()
    .write_c_headers()
    .run_dart_ffigen();
}
```

If everything went as planned you can now call Rust from Dart with `cp ../acme/target/debug/libacme.dylib . && dart --enable-asserts run` (`--enable-asserts` enables a pretty print `toString()` in the generated classes):

``` dart
import 'package:dart_acme/accounts.dart';

void main(List<String> arguments) async {
  var accounts = AccountsApi();
  print(await accounts.contact());
}
```
