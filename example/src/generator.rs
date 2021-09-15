fn main() {
  // make sure the lib.rs doesn't get optimized away during our generator compilation pass
  example::load();

  let mut project = membrane::Membrane::new();
  project
    .package_destination_dir("../dart_example")
    .using_lib("libexample")
    .create_pub_package()
    .write_api()
    .write_c_headers()
    .run_dart_ffigen();
}
