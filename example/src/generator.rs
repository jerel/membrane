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

  let _ = std::fs::create_dir_all("../dart_example/bin");
  let _ = std::fs::write("../dart_example/bin/dart_example.dart", RUNNABLE_EXAMPLE);
}

static RUNNABLE_EXAMPLE: &str = r#"
import 'package:dart_example/accounts.dart';

void main(List<String> arguments) async {
  var accounts = AccountsApi();
  print(await accounts.contact("1"));

  accounts.contacts().forEach((contact) {
    print('Stream item: ' + contact.toString());
  });
}
"#;
