<h1 align="center">Membrane</h1>
<div align="center">
  Membrane is an opinionated crate that generates a Dart package from your Rust library. It provides extremely fast performance with strict typing, automatic memory management, and zero copy returns over the FFI boundary via bincode.
</div>

<br />

<div align="center">
  <a href="https://github.com/jerel/membrane">
    <img src="https://github.com/jerel/membrane/workflows/Tests/badge.svg"
      alt="Tests" />
  </a>
  <a href="https://github.com/jerel/membrane">
    <img src="https://github.com/jerel/membrane/workflows/Clippy%20%26%20Format/badge.svg"
      alt="Lints" />
  </a>
  <a href="https://github.com/jerel/membrane">
    <img src="https://github.com/jerel/membrane/workflows/Valgrind%20Memory%20Check/badge.svg"
      alt="Valgrind Memory Check" />
  </a>
</div>

<h1 align="center"></h1>

![Membrane diagram](https://user-images.githubusercontent.com/322706/138164299-6a29158e-3d52-4981-a7b6-a3bfc0368823.png)

## Development Environment

* Rust
  * https://rustup.rs
* Dart
  * https://dart.dev/get-dart
* libclang (for generating bindings)
  * Linux
    * `apt-get install libclang-dev`
  * MacOS
    * `brew install llvm`

On Linux ffigen looks for libclang at `/usr/lib/llvm-11/lib/libclang.so` so you may need to symlink to the version specific library: `ln -s /usr/lib/llvm-11/lib/libclang.so.1 /usr/lib/llvm-11/lib/libclang.so`.

## Usage

_View the [example](https://github.com/jerel/membrane/tree/main/example) directory for a runnable example._

In your crate's `lib.rs` add a `RUNTIME` static that will survive for the lifetime of the program. `RUNTIME` must provide a `spawn` function, in this case we're using `tokio`:
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

Then write some code that is annotated with the `#[async_dart]` macro. No need to use C types here, just use Rust `String`, `i64`, `f64`, `bool`, structs, or enums as usual (or with `Option`). The functions can be anywhere in your program and may return either an async `Result<T, E>` or an `impl Stream<Item = Result<T, E>>`:

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

And now you are ready to generate the Dart package. Note that this code goes in a `bin/generator.rs` or similar to be ran with `cargo run` or a build task rather than in `build.rs` (which only runs before compilation):

``` rust
fn main() {
  // if nothing else in this generator.rs references lib.rs then
  // at least call a dummy function so lib.rs doesn't get optimized away
  example::load();

  let mut project = membrane::Membrane::new();
  project
    // name the output pub directory
    .package_destination_dir("../dart_example")
    // the pub package name, if different than the directory
    .package_name("example")
    // give the basename of the .so or .dylib that your Rust program provides
    .using_lib("libexample")
    // use Dart enums instead of class enums
    .with_c_style_enums(true)
    .create_pub_package()
    .write_api()
    .write_c_headers()
    .write_bindings();
}
```

If everything went as planned you can now call Rust from Dart with:

``` bash
cd example
cargo run
cargo build
cd ../dart_example
cp ../example/target/debug/libexample.dylib .
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

If you get an error on Linux about not being able to load `libexample.so` then add the pub package's path to `LD_LIBRARY_PATH`.
