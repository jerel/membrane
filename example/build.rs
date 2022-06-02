fn main() {
  #[cfg(feature = "c-example")]
  {
    let headers = std::path::Path::new("./c/");

    cc::Build::new()
      .file("./c/threading_example.c")
      .include(headers)
      .compile("threading_example");
  }
}
