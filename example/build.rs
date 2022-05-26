fn main() {
  let headers = std::path::Path::new("./");

  cc::Build::new()
    .file("threading_example.c")
    .include(headers)
    .compile("threading_example");
}
