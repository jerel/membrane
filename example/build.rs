fn main() {
  #[cfg(feature = "c-example")]
  {
    let headers = std::path::Path::new("./c/");

    cc::Build::new()
      .file("./c/threading_example.c")
      .file("./c/render_example.c")
      .include(headers)
      .compile("c_example");
  }
}
