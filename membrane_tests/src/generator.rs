fn main() {
  // make sure the lib.rs doesn't get optimized away during our generator compilation pass
  membrane_tests::load();

  let mut project = membrane::Membrane::new();
  project
    .timeout(200)
    .package_destination_dir("../dart_tests")
    .package_name("dart_tests")
    .using_lib("libmembrane_tests")
    .create_pub_package()
    .write_api()
    .write_c_headers()
    .write_bindings();

  let _ = std::fs::create_dir_all("../dart_tests/bin");
  let _ = std::fs::write("../dart_tests/bin/dart_tests.dart", RUNNABLE_EXAMPLE);
}

static RUNNABLE_EXAMPLE: &str = r#"
import 'package:dart_tests/accounts.dart';
import 'package:logging/logging.dart';

void main(List<String> arguments) async {
  Logger.root.level = Level.ALL;
  Logger.root.onRecord.listen((event) {
    print(event);
  });

  Logger('membrane_tests').info('Starting dart_tests application');

  var accounts = AccountsApi();
  var one = await accounts.contact(userId: "1");
  print('Item: ' + one.toString());
  var updated = await accounts.updateContact(
      id: "1",
      contact: Contact(id: 1, fullName: "Alice Smith", status: Status.pending),
      sendEmail: true);
  print('Updated: ' + updated.toString());

  try {
    await accounts.deleteContact(id: "1");
  } on AccountsApiError catch (err) {
    print(err.e);
  }

  accounts.contacts().take(1).forEach((contact) {
    print('Stream item: ' + contact.toString());
  });
}
"#;
